// This is to examine the performance cost of the Store

use super::header::{QueryError, Store};
use crate::{Attribute, QuerySection, Save, XHtmlElement, mut_prt_unchecked};

pub struct FakeStore {}

impl<'html, 'query: 'html> Store<'html, 'query> for FakeStore {
    type E = usize;
    type Context = bool;
    fn new(context: Self::Context) -> Self {
        Self {}
    }
    fn root(&mut self) -> *mut Self::E {
        std::ptr::null_mut()
    }
    fn push<'key>(
        &mut self,
        selection: &QuerySection<'query>,
        from: *mut Self::E,
        element: XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>> {
        Ok(std::ptr::null_mut())
    }
    fn set_content(
        &mut self,
        element: *mut Self::E,
        inner_html: Option<&'html str>,
        text_content: Option<String>,
    ) -> () {
        ()
    }
}
