use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{Query, Save};

use napi::Result;
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object, js_name = "Save")]
#[derive(Clone, Copy, Debug)]
pub struct JsSave {
    pub inner_html: bool,
    pub text_content: bool,
}

#[napi]
impl JsSave {
    #[napi]
    pub fn only_inner_html() -> Self {
        Self {
            inner_html: true,
            text_content: false,
        }
    }

    #[napi]
    pub fn only_text_content() -> Self {
        Self {
            inner_html: false,
            text_content: true,
        }
    }

    #[napi]
    pub fn all() -> Self {
        Self {
            inner_html: true,
            text_content: true,
        }
    }

    #[napi]
    pub fn none() -> Self {
        Self {
            inner_html: false,
            text_content: false,
        }
    }

    #[napi]
    pub fn new(inner_html: Option<bool>, text_content: Option<bool>) -> Self {
        Self {
            inner_html: inner_html.unwrap_or(false),
            text_content: text_content.unwrap_or(false),
        }
    }

    fn to_save(self) -> Save {
        Save {
            inner_html: self.inner_html,
            text_content: self.text_content,
        }
    }
}

#[napi(js_name = "QueryBuilder")]
pub struct JsQueryBuilder {
    builder: LazyQueryBuilder<String>,
}

#[napi]
impl JsQueryBuilder {
    #[napi]
    pub fn all(&mut self, selector: String, save: JsSave) -> JsQueryBuilder {
        self.builder.all_mut(selector, save.to_save());

        JsQueryBuilder {
            builder: self.builder.clone(),
        }
    }
    #[napi]
    pub fn first(&mut self, selector: String, save: JsSave) -> JsQueryBuilder {
        self.builder.first_mut(selector, save.to_save());

        JsQueryBuilder {
            builder: self.builder.clone(),
        }
    }

    #[napi]
    pub fn then(
        &mut self,
        callback: Function<JsQueryFactory, Vec<Reference<JsQueryBuilder>>>,
    ) -> Result<JsQueryBuilder> {
        let factory = JsQueryFactory { _data: true };
        let builders = callback.call(factory)?;
        let children = builders.iter().map(|b| b.builder.clone());

        let current_index = self.builder.len() - 1;
        for child in children {
            self.builder.append(current_index, child);
        }

        Ok(JsQueryBuilder {
            builder: self.builder.clone(),
        })
    }

    #[napi]
    pub fn build(&self) -> JsQuery {
        let (_tape, query) = unsafe { self.builder.clone().to_query() };
        JsQuery { _tape, query }
    }
}

#[napi(js_name = "QueryFactory")]
pub struct JsQueryFactory {
    // if their isn't any data in the struct, no object is created, thus the `.then` doesn't work
    _data: bool,
}

#[napi]
impl JsQueryFactory {
    #[napi]
    pub fn all(&self, selector: String, save: JsSave) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::all(selector, save.to_save()),
        }
    }

    #[napi]
    pub fn first(&self, selector: String, save: JsSave) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::first(selector, save.to_save()),
        }
    }
}

#[napi]
#[derive(Clone)]
pub struct JsQuery {
    _tape: std::sync::Arc<Vec<u8>>,
    pub(super) query: Query<'static>,
}

#[napi(js_name = "Query")]
pub struct JsQueryStatic;

#[napi]
impl JsQueryStatic {
    #[napi]
    pub fn all(selector: String, save: JsSave) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::all(selector, save.to_save()),
        }
    }

    #[napi]
    pub fn first(selector: String, save: JsSave) -> JsQueryBuilder {
        JsQueryBuilder {
            builder: LazyQuery::first(selector, save.to_save()),
        }
    }
}
