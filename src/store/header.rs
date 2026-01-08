use crate::XHtmlElement;
use crate::css::QuerySection;

pub trait Store<'html, 'query> {
    type E;
    type Context;
    fn new(context: Self::Context) -> Self;
    fn root(&mut self) -> *mut Self::E;
    fn push<'key>(
        &mut self,
        selection: &QuerySection<'query>,
        from: *mut Self::E,
        element: XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>>;
    fn set_content(
        &mut self,
        element: *mut Self::E,
        inner_html: Option<&'html str>,
        text_content: Option<String>,
    ) -> ();
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError<'key> {
    KeyNotFound(&'key str),
    NotASingleElement,
    NotAList,
    IndexOutOfBounds { index: usize, len: usize },
}
