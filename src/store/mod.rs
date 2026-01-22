mod fake;
mod rust;

pub use rust::Element;
pub(crate) use rust::RustStore;
pub use rust::{Child, ChildIndex};

pub(crate) use fake::FakeStore;

use crate::css::QuerySection;
use crate::runner::element::XHtmlElement;

pub trait Store<'html, 'query> {
    type E;
    type Context;
    fn new(context: Self::Context) -> Self;
    fn push(
        &mut self,
        selection: &QuerySection<'query>,
        from: Self::E,
        element: XHtmlElement<'html>,
    ) -> Self::E;
    fn set_content(
        &mut self,
        element: Self::E,
        inner_html: Option<&'html str>,
        text_content: Option<String>,
    );
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError<'key> {
    KeyNotFound(&'key str),
    NotASingleElement,
    NotAList,
    IndexOutOfBounds { index: usize, len: usize },
}

pub const ROOT: usize = 0;
