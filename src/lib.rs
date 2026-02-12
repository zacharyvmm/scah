mod css;
mod scanner;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;

pub use css::lazy;
pub use css::{FsmManager, Query, QueryBuilder, QueryFactory, Save, Selection, SelectionKind};
pub use store::{ChildIndex, Element, QueryError, Store};
pub use utils::Reader;
pub use xhtml::element::element::{Attribute, XHtmlElement};
pub use xhtml::parser::XHtmlParser;

pub fn parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> Store<'html, 'query> {
    let selectors = FsmManager::new(queries);
    let mut parser = XHtmlParser::with_capacity(selectors, html.len());

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    parser.matches()
}
