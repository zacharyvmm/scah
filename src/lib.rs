mod css;
mod store;
mod utils;
mod xhtml;

use utils::Reader;
pub use xhtml::element::element::{Attribute, XHtmlElement};
use xhtml::parser::XHtmlParser;
pub use xhtml::text_content::TextContent;

use crate::css::FsmManager;

/*
pub use crate::css::{MatchTree, Save, SelectionKind, SelectionPart, Selection};

pub fn parse<'html, 'query>(
    html: &'html str,
    queries: Vec<Selection<'query>>,
) -> (SelectionMap<'query, 'html>, TextContent<'html>) {
    let selectors = Selectors::new(queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    return (parser.selectors.map, parser.content);
}
*/
