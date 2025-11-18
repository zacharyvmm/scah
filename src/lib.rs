mod css;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;
use utils::Reader;
use xhtml::parser::XHtmlParser;

use crate::store::RustStore;
use crate::css::FsmManager;

pub use css::{Save, SelectionKind, SelectionPart, Selection};
pub use xhtml::element::element::{Attribute, XHtmlElement};
pub use store::{QueryError, SelectionValue, Element};

pub fn parse<'html:'query, 'query:'html>(
    html: &'html str,
    queries: &'query Vec<Selection<'query>>,
) -> HashMap<&'query str, SelectionValue<'html, 'query>> {
    let selectors = FsmManager::<RustStore>::new(queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    return parser.matches().root.children;
}