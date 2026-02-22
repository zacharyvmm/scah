use napi::bindgen_prelude::*;
use napi::{Env, Result};
use napi_derive::napi;

use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{FsmManager, Query, Reader, Save, XHtmlParser};

use std::ops::Range;

fn buffer_slice_from_raw<'a>(env: &Env, data_ptr: *mut u8, len: usize) -> Result<BufferSlice<'a>> {
  unsafe {
    BufferSlice::from_external(env, data_ptr, len, data_ptr, move |_, _ptr| {
      // do nothing
    })
  }
}

pub type RequiredField = Range<usize>;
pub type OptionalField = Option<Range<usize>>;

pub struct TapeElement {
  pub(crate) name: RequiredField,
  pub(crate) class: OptionalField,
  pub(crate) id: OptionalField,
  pub(crate) attributes: Vec<(RequiredField, OptionalField)>,

  pub(crate) inner_html: OptionalField,
  pub(crate) text_content: OptionalField,

  pub(crate) children: Vec<(RequiredField, Vec<i64>)>,
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

#[napi(js_name = "Store")]
pub struct JSStore {
  pub(crate) elements: Vec<TapeElement>,

  // So the text content gets cleaned up when the store is dropped
  pub(crate) text_content: Buffer,
  pub(crate) html: Buffer,
  pub(crate) query_tape: Buffer,
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
