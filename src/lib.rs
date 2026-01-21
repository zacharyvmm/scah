mod css;
mod indexer;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;

use crate::store::{FakeStore, RustStore};

pub use css::{FsmManager, Query, QueryBuilder, QuerySection, Save, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, Store};
pub use utils::Reader;
pub use xhtml::element::element::{Attribute, XHtmlElement};
pub use xhtml::parser::XHtmlParser;

pub fn parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> Vec<Element<'html, 'query>> {
    let selectors = FsmManager::new(RustStore::new(()), queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    parser.matches().arena
}

pub fn fake_parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) {
    let selectors = FsmManager::new(FakeStore::new(()), queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    let _ = parser.matches();
}
