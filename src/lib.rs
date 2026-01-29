mod css;
mod scanner;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;

pub use css::{FsmManager, Query, QueryBuilder, QuerySection, Save, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, Store};
pub use utils::Reader;
pub use xhtml::element::element::{Attribute, XHtmlElement};
pub use xhtml::parser::XHtmlParser;

pub fn parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> Store<'html, 'query> {
    let selectors = FsmManager::new(queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    parser.matches()
}
