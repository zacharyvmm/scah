use ::scah::{Attribute, ElementId, Store};

use napi::bindgen_prelude::*;
use napi::{Env, Error, Result, Status};
use napi_derive::napi;

#[napi(object)]
/// A matched element as a plain object.
pub struct JsonElement<'a> {
    pub name: String,
    pub id: Option<String>,
    pub class: Option<String>,
    pub attributes: Object<'a>,
    pub inner_html: Option<String>,
    pub raw_text: Option<String>,
    pub text: Option<String>,
}

/// A matched element. Content fields are `null` when the query did not
/// save them.
#[napi(js_name = "Element")]
pub struct JsElement {
    pub(super) store: std::sync::Arc<Store<'static, 'static>>,
    pub(super) id: ElementId,
}

#[napi]
impl JsElement {
    #[napi]
    /// This element as a plain object.
    pub fn to_json<'a>(&'a self, env: Env) -> Result<JsonElement<'a>> {
        let element = self
            .store
            .element(self.id)
            .expect("The Element ID should be valid");

        let json = JsonElement {
            name: element.name().to_string(),
            id: element.id().map(|s| s.to_string()),
            class: element.class().map(|s| s.to_string()),
            attributes: self.attributes(env)?,
            inner_html: element.inner_html().map(|s| s.to_string()),
            raw_text: element.raw_text().map(|s| s.to_string()),
            text: element.text().map(|s| s.to_string()),
        };

        Ok(json)
    }

    #[napi(getter)]
    /// The tag name.
    pub fn name(&self) -> Option<&str> {
        self.store.element(self.id).map(|e| e.name())
    }

    #[napi(getter)]
    /// The `class` attribute.
    pub fn class_name(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.class())
    }

    #[napi(getter)]
    /// The `id` attribute.
    pub fn id(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.id())
    }

    #[napi]
    /// The value of the attribute named `key`, matched case-insensitively.
    pub fn get_attribute(&self, key: String) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.attribute(&key))
    }

    #[napi(getter)]
    /// The attributes other than `id` and `class`.
    pub fn attributes<'a>(&'a self, env: Env) -> Result<Object<'a>> {
        let mut object = Object::new(&env)?;
        let attributes = self.store.element(self.id).and_then(|e| e.attributes());

        if let Some(attrs) = attributes {
            for Attribute { key, value } in attrs {
                object.set(key, value)?
            }
        }
        Ok(object)
    }

    #[napi(getter)]
    /// The markup inside the element.
    pub fn inner_html(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.inner_html())
    }

    #[napi(getter)]
    /// The source text of the element's descendants, with markup removed.
    pub fn raw_text(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.raw_text())
    }

    #[napi(getter)]
    /// Normalized, readable text.
    pub fn text(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.text())
    }

    #[napi(getter)]
    /// Deprecated compatibility alias for `text`.
    pub fn text_content(&self) -> Option<&str> {
        self.text()
    }

    #[napi]
    /// This element's matches for the child query `query`. Throws if this
    /// element has no matches for `query`, or `query` is not one of its
    /// child queries.
    pub fn get(&self, query: String) -> Result<Vec<JsElement>> {
        match self.store.child_results(self.id, &query) {
            None => Err(Error::new(
                Status::GenericFailure,
                format!("This Element does not have children selected with `{query}`"),
            )),
            Some(children) => Ok(children
                .iter()
                .map(|&id| JsElement {
                    store: self.store.clone(),
                    id,
                })
                .collect()),
        }
    }
}
