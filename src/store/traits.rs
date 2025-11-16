use crate::{XHtmlElement, store::rust::Element};

pub trait Store<'html, E> {
    fn push(&mut self, from: &'html E, element: XHtmlElement<'html>) -> &E;
}
