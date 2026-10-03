use napi_derive::napi;

use super::elements::JsElement;
use scah_ffi::OwnedStore;

use std::sync::Arc;

#[napi(js_name = "Store")]
pub struct JSStore {
    pub(crate) store: Arc<OwnedStore>,
}

#[napi]
impl JSStore {
    #[napi]
    pub fn get(&self, query: String) -> Option<Vec<JsElement>> {
        let store = self.store.store();
        store.get(&query).map(|iter| {
            iter.map(|e| unsafe { store.elements.index_of(e) })
                .map(|i| JsElement {
                    store: self.store.clone(),
                    id: i,
                })
                .collect()
        })
    }

    #[napi(getter)]
    pub fn length(&self) -> i64 {
        self.store.store().elements.len() as i64
    }
}
