#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Result};
use napi_derive::napi;

use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{FsmManager, Query, Reader, Save, XHtmlParser};
use std::fmt::Error;
use std::num::NonZeroUsize;
use std::ops::Range;

type RequiredField = Range<usize>;
type OptionalField = Option<Range<usize>>;

pub struct TapeElement {
  name: RequiredField,
  class: OptionalField,
  id: OptionalField,
  attributes: Vec<(RequiredField, OptionalField)>,

  inner_html: OptionalField,
  text_content: OptionalField,

  children: Vec<(RequiredField, Vec<i64>)>,
}

#[napi(object, js_name = "Element")]
pub struct JsElement {
  pub name: Buffer,
  pub class: Option<Buffer>,
  pub id: Option<Buffer>,
  pub attributes: Vec<(Buffer, Option<Buffer>)>,

  pub inner_html: Option<Buffer>,
  pub text_content: Option<Buffer>,

  pub children: Vec<(Buffer, Vec<i64>)>,
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
pub struct JSStore {
  elements: Vec<TapeElement>,

  // So the text content gets cleaned up when the store is dropped
  text_content: Buffer,
  html: Buffer,
  query_tape: Buffer,
}

#[napi]
impl JSStore {
  fn get_raw_from_field(source: &[u8], slice: RequiredField) -> (*const u8, usize) {
    let ptr = source.as_ptr();
    let start = unsafe { ptr.add(slice.start) };
    (start, slice.end - slice.start)
  }

  fn convert_element(&self, env: &Env, e: &TapeElement) -> JsElement {
    let range_from_html = |range| -> Buffer {
      let (ptr, len) = Self::get_raw_from_field(&self.html, range);
      buffer_slice_from_raw(env, ptr as *mut u8, len)
        .unwrap()
        .into_buffer(env)
        .unwrap()
    };
    JsElement {
      name: range_from_html(e.name.clone()),
      id: e.id.as_ref().map(|r| range_from_html(r.clone())),
      class: e.class.as_ref().map(|r| range_from_html(r.clone())),
      inner_html: e.inner_html.as_ref().map(|r| range_from_html(r.clone())),
      text_content: e.text_content.as_ref().map(|r| {
        let (ptr, len) = Self::get_raw_from_field(&self.text_content, r.clone());
        buffer_slice_from_raw(env, ptr as *mut u8, len)
          .unwrap()
          .into_buffer(env)
          .unwrap()
      }),
      attributes: e
        .attributes
        .iter()
        .map(|(k, v)| {
          (
            range_from_html(k.clone()),
            v.as_ref().map(|r| range_from_html(r.clone())),
          )
        })
        .collect(),
      children: e
        .children
        .iter()
        .map(|(k, v)| {
          (
            {
              let (ptr, len) = Self::get_raw_from_field(&self.query_tape, k.clone());
              buffer_slice_from_raw(env, ptr as *mut u8, len)
                .unwrap()
                .into_buffer(env)
                .unwrap()
            },
            v.clone(),
          )
        })
        .collect(),
    }
  }

  #[napi]
  pub fn get(&self, env: &Env, index: u32) -> Option<JsElement> {
    self
      .elements
      .get(index as usize)
      .map(|e| self.convert_element(env, e))
  }
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
  parent_range == subslice_range
    || (parent_range.contains(&subslice_range.start) && parent_range.contains(&subslice_range.end))
}

fn buffer_slice_from_raw<'a>(env: &Env, data_ptr: *mut u8, len: usize) -> Result<BufferSlice<'a>> {
  unsafe {
    BufferSlice::from_external(env, data_ptr, len, data_ptr, move |_, _ptr| {
      // do nothing
    })
  }
}

fn range_from_str_slice<'a>(base: &'a [u8], slice: &'a str) -> Range<usize> {
  if slice == "root" {
    return 0..0;
  }
  assert!(is_subslice(
    &base.as_ptr_range(),
    &(slice.as_ptr() as *const u8..unsafe { slice.as_ptr().add(slice.len()) })
  ));
  let start = slice.as_ptr() as usize - base.as_ptr() as usize;

  start..start + slice.len()
}

#[napi]
fn parse<'a>(
  env: &'a Env,
  html: BufferSlice<'a>,
  queries: Vec<Reference<JSQuery>>,
) -> Result<JSStore> {
  if queries.is_empty() {
    return Err(napi::Error::new(
      napi::Status::ArrayExpected,
      "No queries where passed".to_owned(),
    ));
  }

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

  let full_tape = queries
    .iter()
    .map(|q| (*q.tape).clone())
    .collect::<Vec<_>>()
    .concat();

  let full_tape_slice = unsafe { std::slice::from_raw_parts(full_tape.as_ptr(), full_tape.len()) };

  let find_query = |query: &str| -> Range<usize> {
    let query_ptr = query.as_ptr();
    let query_len = query.as_bytes().len();
    // println!("{query}, {query_ptr:?}, {query_len}");
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
          range_from_str_slice(html_bytes, a.key),
          a.value.map(|v| range_from_str_slice(html_bytes, v)),
        ))
      })
      .collect::<Result<Vec<(RequiredField, OptionalField)>>>()?;

    let children = element
      .children
      .iter()
      .filter_map(|c| {
        // TODO: Convert ChildIndex to Vec<usize> if needed, currently passing empty vec
        Some((
          find_query(c.query),
          c.index.vec().into_iter().map(|i| i as i64).collect(),
        ))
      })
      .collect::<Vec<(Range<usize>, Vec<i64>)>>();

    elements.push(TapeElement {
      name: range_from_str_slice(html_bytes, element.name),
      class: element.class.map(|r| range_from_str_slice(html_bytes, r)),
      id: element.id.map(|r| range_from_str_slice(html_bytes, r)),
      attributes,
      inner_html: element
        .inner_html
        .map(|r| range_from_str_slice(html_bytes, r)),
      text_content: element.text_content,
      children,
    });
  }

  Ok(JSStore {
    elements,
    text_content: text_content.into_buffer(env).unwrap(),
    html: html.into_buffer(env).unwrap(),
    query_tape: full_tape.into(),
  })
}
