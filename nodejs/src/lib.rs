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

  let slice = unsafe { std::slice::from_raw_parts(queries_rs.as_ptr(), queries_rs.len()) };
  let selectors = QueryMultiplexer::new(slice);
  let mut parser = XHtmlParser::with_capacity(selectors, html_bytes.len());
  let mut reader = Reader::from_bytes(html_bytes);
  while parser.next(&mut reader) {}

  let store = std::sync::Arc::new(parser.matches());

  Ok(JSStore { store, _html: html })
}
