mod css;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;

use crate::store::{FakeStore, RustStore};

pub use css::{FsmManager, Query, QueryBuilder, QuerySection, Save, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, SelectionValue, Store};
pub use utils::Reader;
pub use xhtml::element::element::{Attribute, XHtmlElement};
pub use xhtml::parser::XHtmlParser;

pub fn parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> HashMap<&'query str, SelectionValue<'html, 'query>> {
    let selectors = FsmManager::new(RustStore::new(false), queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    return parser.matches().root.children;
}

pub fn fake_parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> () {
    let selectors = FsmManager::new(FakeStore::new(false), queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    let _ = parser.matches();
}
