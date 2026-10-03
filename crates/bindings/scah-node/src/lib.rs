use napi::Result;
use napi::bindgen_prelude::*;
use napi_derive::napi;

use ::scah::ParseError;
use ::scah_ffi::OwnedStore;

use std::sync::Arc;

mod query;
use query::JsQuery;
mod store;
use store::JSStore;

mod elements;

#[napi]
#[allow(dead_code)]
fn parse(html: String, queries: Vec<Reference<JsQuery>>) -> Result<JSStore> {
    let queries: Vec<_> = queries.iter().map(|q| &q.query).collect();
    let store = match OwnedStore::parse(html, &queries) {
        Ok(store) => store,
        Err(ParseError::EmptyQueries) => {
            return Err(napi::Error::new(
                napi::Status::ArrayExpected,
                "parse requires at least one query".to_owned(),
            ));
        }
        Err(ParseError::MaximumDepthExceeded) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "HTML nesting depth exceeds the maximum supported depth".to_owned(),
            ));
        }
        Err(ParseError::TextCaptureRequired) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "internal error: unexpected TextCaptureRequired from parse".to_owned(),
            ));
        }
    };

    Ok(JSStore {
        store: Arc::new(store),
    })
}
