use napi_derive::napi;

use super::elements::JsElement;
use scah::{ElementId, Store};

use std::sync::Arc;

#[napi(js_name = "Store")]
pub struct JSStore {
    pub(crate) store: Arc<Store<'static, 'static>>,
    pub(crate) _html: Arc<String>,
    pub(crate) _query_tapes: Vec<Arc<Vec<u8>>>,
}

#[napi]
impl JSStore {
    /// Results of the first query whose selector is `query`.
    #[napi]
    pub fn get(&self, query: String) -> Option<Vec<JsElement>> {
        self.store.results(&query).map(|ids| self.elements(ids))
    }

    /// Results of the query at `index` in the list given to `parse`.
    #[napi]
    pub fn query(&self, index: u32) -> Option<Vec<JsElement>> {
        self.store
            .query_results(index as usize)
            .map(|ids| self.elements(ids))
    }

    fn elements(&self, ids: &[ElementId]) -> Vec<JsElement> {
        ids.iter()
            .map(|&id| JsElement {
                store: self.store.clone(),
                id,
            })
            .collect()
    }

    #[napi(getter)]
    pub fn length(&self) -> i64 {
        self.store.elements.len() as i64
    }
}
