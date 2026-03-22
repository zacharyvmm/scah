#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Result};
use napi_derive::napi;

use ::scah::{Query, QueryMultiplexer, Reader, Save, XHtmlParser};

use std::ops::Range;

mod query;
use query::JsQuery;
mod store;
use store::JSStore;

mod elements;

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
fn parse(html: String, queries: Vec<Reference<JsQuery>>) -> Result<JSStore> {
  if queries.is_empty() {
    return Err(napi::Error::new(
      napi::Status::ArrayExpected,
      "No queries where passed".to_owned(),
    ));
  }

  let len = html.as_bytes().len();
  let html_bytes = unsafe { std::slice::from_raw_parts(html.as_ptr(), len) };
  let queries_rs = queries
    .iter()
    .map(|q| q.query.clone())
    .collect::<Vec<Query>>();

  // This is SAFE because the query strings are owned by the JSQuery objects,
  // and those are in the Arc<Vec<u8>>.
  let slice = unsafe { std::slice::from_raw_parts(queries_rs.as_ptr(), queries_rs.len()) };
  let selectors = QueryMultiplexer::new(slice);
  let mut parser = XHtmlParser::with_capacity(selectors, html_bytes.len());
  let mut reader = Reader::from_bytes(html_bytes);
  while parser.next(&mut reader) {}

  let store = std::sync::Arc::new(parser.matches());

  Ok(JSStore { store, html })
}
