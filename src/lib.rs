mod css;
mod runner;
mod scanner;
mod store;
mod utils;
mod xhtml;

use crate::{
    runner::Runner,
    store::{FakeStore, RustStore},
};

pub use css::{FsmManager, Query, QueryBuilder, QuerySection, Save, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, Store};
pub use utils::Reader;
//pub use xhtml::element::element::{Attribute, XHtmlElement};
//pub use xhtml::parser::XHtmlParser;

pub fn parse<'a: 'query, 'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'a [Query<'query>],
) -> Vec<Element<'html, 'query>> {
    //let selectors = FsmManager::new(RustStore::new(()), queries);
    // let mut parser = XHtmlParser::new(selectors);

    // let mut reader = Reader::new(html);
    // while parser.next(&mut reader) {}

    // parser.matches().arena
    Runner::run(html, queries).arena
}
