#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Result};
use napi_derive::napi;

use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{FsmManager, Query, Reader, Save, XHtmlParser};
use std::ops::Range;

#[napi(js_name = "Element")]
pub struct JSElement<'a> {
  name: BufferSlice<'a>,
  class: Option<BufferSlice<'a>>,
  id: Option<BufferSlice<'a>>,
  attributes: Vec<(BufferSlice<'a>, Option<BufferSlice<'a>>)>,

  inner_html: Option<BufferSlice<'a>>,
  text_content: Option<BufferSlice<'a>>,

  children: Vec<(BufferSlice<'a>, Vec<usize>)>,
}

#[napi(js_name = "Query")]
pub struct JSQuery {
  tape: std::sync::Arc<Vec<u8>>,
  query: Query<'static>,
}

#[napi]
impl<'a> JSQuery {
  #[napi(factory)]
  pub fn all(selector: String) -> Self {
    println!("all: {}", selector);
    let (tape, query) = unsafe { LazyQuery::all(selector, Save::all()).to_query() };
    Self { tape, query }
  }
}

#[napi(js_name = "Store")]
pub struct JSStore<'a> {
  elements: Vec<JSElement<'a>>,

  // So the text content gets cleaned up when the store is dropped
  text_content: BufferSlice<'a>,
  query_tape: std::sync::Arc<Vec<u8>>,
}

// https://napi.rs/docs/concepts/typed-array.en#external-buffers
fn create_external_buffer(env: &Env, mut data: Vec<u8>) -> Result<BufferSlice<'_>> {
  let data_ptr = data.as_mut_ptr();
  let capacity = data.capacity();

  // make sure the data is valid until the finalize callback is called
  std::mem::forget(data);

  unsafe {
    BufferSlice::from_external(env, data_ptr, capacity, data_ptr, move |_, ptr| {
      // Cleanup data when JavaScript GC runs
      std::mem::drop(Vec::from_raw_parts(ptr, capacity, capacity));
    })
  }
}

pub fn is_subslice<T>(
  parent_range: &std::ops::Range<*const T>,
  subslice_range: &std::ops::Range<*const T>,
) -> bool {
  parent_range.contains(&subslice_range.start) && parent_range.contains(&subslice_range.end)
}

fn buffer_slice_from_raw<'a>(env: &Env, data_ptr: *mut u8, len: usize) -> Result<BufferSlice<'a>> {
  unsafe {
    BufferSlice::from_external(env, data_ptr, len, data_ptr, move |_, _ptr| {
      // do nothing
    })
  }
}

fn buffer_slice_from_slice<'a>(
  env: &Env,
  data: &'a [u8],
  slice: Range<usize>,
) -> Result<BufferSlice<'a>> {
  let sliced_data = &data[slice];
  let data_ptr = sliced_data.as_ptr() as *mut u8;
  let len = sliced_data.len();

  buffer_slice_from_raw(env, data_ptr, len)
}

fn buffer_slice_from_str_slice<'a>(
  env: &Env,
  base: &'a [u8],
  slice: &'a str,
) -> Result<BufferSlice<'a>> {
  assert!(is_subslice(
    &base.as_ptr_range(),
    &(slice.as_ptr() as *const u8..unsafe { slice.as_ptr().add(slice.len()) })
  ));
  let start = slice.as_ptr() as usize - base.as_ptr() as usize;
  buffer_slice_from_slice(env, base, start..start + slice.len())
}

#[napi]
fn parse<'a>(
  env: &'a Env,
  html: BufferSlice<'a>,
  queries: Vec<Reference<JSQuery>>,
) -> Result<JSStore<'a>> {
  let html_bytes = unsafe { std::slice::from_raw_parts(html.as_ptr() as *const u8, html.len()) };
  let queries_rs = queries
    .iter()
    .map(|q| q.query.clone())
    .collect::<Vec<Query>>();

  // This is SAFE because the query strings are owned by the JSQuery objects,
  // and those are in the Arc<Vec<u8>>.
  let slice = unsafe { std::slice::from_raw_parts(queries_rs.as_ptr(), queries_rs.len()) };
  let selectors = FsmManager::new(slice);
  let mut parser = XHtmlParser::with_capacity(selectors, html_bytes.len());
  let mut reader = Reader::from_bytes(html_bytes);
  while parser.next(&mut reader) {}

  let store = parser.matches();

  let data = store.text_content.data();
  let text_content = create_external_buffer(env, data)?;
  let text_content_ptr = text_content.as_ptr();

  let mut elements = Vec::with_capacity(store.elements.len());

  let mut full_tape_position = 0;
  let pointer_range = queries
    .iter()
    .map(|q| {
      let start = full_tape_position.clone();
      full_tape_position += q.tape.len();
      (start, (*q.tape).as_ptr_range())
    })
    .collect::<Vec<_>>();

  let full_tape = std::sync::Arc::new(
    queries
      .iter()
      .map(|q| (*q.tape).clone())
      .collect::<Vec<_>>()
      .concat(),
  );

  let full_tape_slice = unsafe { std::slice::from_raw_parts(full_tape.as_ptr(), full_tape.len()) };

  let find_query = |query: &str| -> Range<usize> {
    let query_ptr = query.as_ptr();
    let query_len = query.as_bytes().len();
    pointer_range
      .clone()
      .into_iter()
      .find(|(_, ptr_range)| {
        let end = unsafe { ptr_range.start.add(query_len) };
        is_subslice(ptr_range, &(query_ptr..end))
      })
      .map(|(start, ptr_range)| {
        let start = start + (unsafe { query_ptr.offset_from(ptr_range.start) }) as usize;
        start..(start + query_len)
      })
      .unwrap()
  };

  for element in store.elements.into_iter() {
    let attributes = element
      .attributes
      .iter()
      .map(|a| {
        Ok((
          buffer_slice_from_str_slice(env, html_bytes, a.key)?,
          a.value
            .map(|v| buffer_slice_from_str_slice(env, html_bytes, v))
            .transpose()?,
        ))
      })
      .collect::<Result<Vec<(BufferSlice, Option<BufferSlice>)>>>()?;

    // let children = element
    //   .children
    //   .iter()
    //   .filter_map(|c| {
    //     // TODO: Convert ChildIndex to Vec<usize> if needed, currently passing empty vec
    //     if c.query.is_valid() {
    //       return None;
    //     }
    //     Some((
    //       buffer_slice_from_slice(env, full_tape_slice, find_query(c.query)).unwrap(),
    //       vec![],
    //     ))
    //   })
    //   .collect::<Vec<(BufferSlice, Vec<usize>)>>();
    let children = vec![];

    elements.push(JSElement {
      name: buffer_slice_from_str_slice(env, html_bytes, element.name).unwrap(),
      class: element
        .class
        .map(|r| buffer_slice_from_str_slice(env, html_bytes, r))
        .transpose()?,
      id: element
        .id
        .map(|r| buffer_slice_from_str_slice(env, html_bytes, r))
        .transpose()?,
      attributes,
      inner_html: element
        .inner_html
        .map(|r| buffer_slice_from_str_slice(env, html_bytes, r))
        .transpose()?,
      text_content: element
        .text_content
        .map(|r| {
          buffer_slice_from_raw(
            env,
            unsafe { text_content_ptr.add(r.start) } as *mut u8,
            r.end - r.start,
          )
        })
        .transpose()?,
      children,
    });
  }

  Ok(JSStore {
    elements,
    text_content,
    query_tape: full_tape,
  })
}
