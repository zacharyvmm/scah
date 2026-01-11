use crate::XHtmlElement;
use crate::css::QuerySection;

pub trait Store<'html, 'query> {
    type E;
    type Context;
    fn new(context: Self::Context) -> Self;
    fn push(
        &mut self,
        selection: &QuerySection<'query>,
        from: usize,
        element: XHtmlElement<'html>,
    ) -> usize;
    fn set_content(
        &mut self,
        element: usize,
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
