// This is to examine the performance cost of the Store

use super::Store;
use crate::{QuerySection, XHtmlElement};

pub struct FakeStore {}

impl<'html, 'query: 'html> Store<'html, 'query> for FakeStore {
    type E = usize;
    type Context = bool;
    fn new(_context: Self::Context) -> Self {
        Self {}
    }
    fn push(
        &mut self,
        _selection: &QuerySection<'query>,
        _from: usize,
        _element: XHtmlElement<'html>,
    ) -> usize {
        0
    }
    fn set_content(
        &mut self,
        _element: usize,
        _inner_html: Option<&'html str>,
        _text_content: Option<String>,
    ) {
    }
}
