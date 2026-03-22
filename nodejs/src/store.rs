use napi::bindgen_prelude::*;
use napi::{Env, Result};
use napi_derive::napi;

use super::elements::JsElement;
use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{Query, QueryMultiplexer, Reader, Save, XHtmlParser};
use scah::Store;

use std::ops::Range;

fn buffer_slice_from_raw<'a>(env: &Env, data_ptr: *mut u8, len: usize) -> Result<BufferSlice<'a>> {
  unsafe {
    BufferSlice::from_external(env, data_ptr, len, data_ptr, move |_, _ptr| {
      // do nothing
    })
  }
}

#[napi(js_name = "Store")]
pub struct JSStore {
  pub(crate) store: std::sync::Arc<Store<'static, 'static>>,
  pub(crate) html: String,
}

#[napi]
impl JSStore {
  #[napi]
  pub fn get(&self, query: String) -> Option<Vec<JsElement>> {
    self.store.get(&query).map(|iter| {
      iter
        .map(|e| unsafe { self.store.elements.index_of(e) })
        .map(|i| JsElement {
          store: self.store.clone(),
          id: i,
        })
        .collect()
    })
  }

  #[napi(getter)]
  pub fn length(&self) -> i64 {
    self.store.elements.len() as i64
  }
}
