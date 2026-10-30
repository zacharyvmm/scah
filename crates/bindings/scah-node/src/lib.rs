use napi::Result;
use napi::bindgen_prelude::*;
use napi_derive::napi;

use ::scah::ParseError;
use ::scah_ffi::{OwnedQuery, OwnedStore};

use std::sync::Arc;

mod query;
use query::JsQuery;
mod store;
use store::JSStore;

mod elements;

#[napi]
#[allow(dead_code)]
fn parse(html: String, queries: Vec<Reference<JsQuery>>) -> Result<JSStore> {
    let queries: Vec<&OwnedQuery> = queries.iter().map(|q| &q.query).collect();
    let store = OwnedStore::parse(html, &queries).map_err(parse_error)?;
    Ok(JSStore {
        store: Arc::new(store),
    })
}

/// The JavaScript error for a failed parse.
fn parse_error(err: ParseError) -> Error {
    match err {
        ParseError::EmptyQueries => Error::new(Status::ArrayExpected, err.to_string()),
        ParseError::MaximumDepthExceeded => Error::new(Status::GenericFailure, err.to_string()),
        ParseError::InputTooLarge => Error::new(Status::InvalidArg, err.to_string()),
        // Only `parse_without_text_capture` reports this.
        ParseError::TextCaptureRequired => Error::new(
            Status::GenericFailure,
            "internal error: unexpected TextCaptureRequired from parse".to_owned(),
        ),
    }
}
