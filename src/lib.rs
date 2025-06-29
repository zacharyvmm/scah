mod css;
mod utils;
mod xhtml;

pub use css::selection_map::{BodyContent, SelectionMap};
use css::selectors::Selectors;
pub use css::selectors::{InnerContent, SelectorQuery, SelectorQueryKind};
use utils::reader::Reader;
use xhtml::parser::XHtmlParser;
pub use xhtml::text_content::TextContent;
pub use xhtml::element::element::{XHtmlElement, Attribute};

pub fn parse<'html, 'query>(
    html: &'html str,
    queries: Vec<SelectorQuery<'query>>,
) -> (SelectionMap<'query, 'html>, TextContent<'html>) {
    let selectors = Selectors::new(queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    return (parser.selectors.map, parser.content);
}
