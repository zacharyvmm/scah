use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{QuerySectionId, Save};
use ::scah_ffi::OwnedQuery;

use napi::Result;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// What to keep for each match of a query. Every field is optional; by
/// default only the element's name and attributes are kept.
#[napi(object, js_name = "Save")]
#[derive(Clone, Copy, Debug)]
pub struct JsSave {
    /// Keep the markup inside the element. Default `false`.
    pub inner_html: Option<bool>,
    /// Deprecated compatibility alias for `text`.
    pub text_content: Option<bool>,
    /// Keep the element's id, class, and other attributes. Default `true`.
    pub attributes: Option<bool>,
    /// Keep the source text of the element's descendants, with markup removed.
    /// Default `false`.
    pub raw_text: Option<bool>,
    /// Keep normalized, readable text: references decoded, hidden content
    /// skipped, and line breaks between blocks. Default `false`.
    pub text: Option<bool>,
}

impl JsSave {
    fn none() -> Self {
        Self {
            inner_html: Some(false),
            text_content: Some(false),
            attributes: Some(true),
            raw_text: Some(false),
            text: Some(false),
        }
    }

    fn to_save(self) -> Save {
        Save {
            inner_html: self.inner_html.unwrap_or(false),
            attributes: self.attributes.unwrap_or(true),
            raw_text: self.raw_text.unwrap_or(false),
            text: self.text.or(self.text_content).unwrap_or(false),
        }
    }
}

/// A query being built. Add child queries with `all`, `first`, or `then`,
/// then call `build`.
#[napi(js_name = "QueryBuilder")]
pub struct JsQueryBuilder {
    builder: LazyQueryBuilder<String>,
}

#[napi]
impl JsQueryBuilder {
    #[napi]
    /// Add a child query that keeps every match of `selector` inside each
    /// match of the last selector.
    pub fn all(&mut self, selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        self.builder
            .all_mut(selector, save.unwrap_or_else(JsSave::none).to_save());

        JsQueryBuilder {
            builder: self.builder.clone(),
        }
    }
    #[napi]
    /// Add a child query that keeps the first match of `selector` inside each
    /// match of the last selector.
    pub fn first(&mut self, selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        self.builder
            .first_mut(selector, save.unwrap_or_else(JsSave::none).to_save());

        JsQueryBuilder {
            builder: self.builder.clone(),
        }
    }

    #[napi]
    /// Add the child queries that `callback(factory)` returns. They match
    /// inside each match of the last selector; build them with `factory.all()`
    /// and `factory.first()`.
    pub fn then(
        &mut self,
        callback: Function<JsQueryFactory, Vec<Reference<JsQueryBuilder>>>,
    ) -> Result<JsQueryBuilder> {
        let factory = JsQueryFactory { _data: true };
        let builders = callback.call(factory)?;
        let children = builders.iter().map(|b| b.builder.clone());

        let current_index = QuerySectionId(self.builder.len() - 1);
        for child in children {
            self.builder.append(current_index, child);
        }

        Ok(JsQueryBuilder {
            builder: self.builder.clone(),
        })
    }

    #[napi]
    /// Finish the query, ready to pass to `parse`.
    ///
    /// Throws if a selector is invalid or uses syntax scah cannot evaluate in
    /// one pass, such as `:has()` or `:last-child`.
    pub fn build(&self) -> Result<JsQuery> {
        let query = OwnedQuery::build(self.builder.clone())
            .map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(JsQuery { query })
    }
}

/// Builds child queries inside a `then` callback.
#[napi(js_name = "QueryFactory")]
pub struct JsQueryFactory {
    // if their isn't any data in the struct, no object is created, thus the `.then` doesn't work
    _data: bool,
}

#[napi]
impl JsQueryFactory {
    #[napi]
    /// A child query that keeps every match of `selector`.
    pub fn all(&self, selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::all(selector, save.unwrap_or_else(JsSave::none).to_save()),
        }
    }

    #[napi]
    /// A child query that keeps the first match of `selector`.
    pub fn first(&self, selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::first(selector, save.unwrap_or_else(JsSave::none).to_save()),
        }
    }
}

#[napi]
#[derive(Clone)]
/// A built query, ready to pass to `parse`. It can be reused across
/// documents.
pub struct JsQuery {
    pub(crate) query: OwnedQuery,
}

/// Entry point for building queries.
#[napi(js_name = "Query")]
pub struct JsQueryStatic;

#[napi]
impl JsQueryStatic {
    #[napi]
    /// Start a query that keeps every element matching `selector`.
    pub fn all(selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::all(selector, save.unwrap_or_else(JsSave::none).to_save()),
        }
    }

    #[napi]
    /// Start a query that keeps the first element matching `selector`.
    pub fn first(selector: String, save: Option<JsSave>) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::first(selector, save.unwrap_or_else(JsSave::none).to_save()),
        }
    }
}
