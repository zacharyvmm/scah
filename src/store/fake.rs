// This is to examine the performance cost of the Store

use super::header::{QueryError, Store};
use crate::{QuerySection, XHtmlElement};

pub struct FakeStore {}

impl<'html, 'query: 'html> Store<'html, 'query> for FakeStore {
    type E = usize;
    type Context = bool;
    fn new(_context: Self::Context) -> Self {
        Self {}
    }
    fn root(&mut self) -> *mut Self::E {
        std::ptr::null_mut()
    }
    fn push<'key>(
        &mut self,
        _selection: &QuerySection<'query>,
        _from: *mut Self::E,
        _element: XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>> {
        Ok(std::ptr::null_mut())
    }
    fn set_content(
        &mut self,
        _element: *mut Self::E,
        _inner_html: Option<&'html str>,
        _text_content: Option<String>,
    ) -> () {
        ()
    }
}
