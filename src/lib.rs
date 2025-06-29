mod css;
mod utils;
mod xhtml;

pub use css::selection_map::{BodyContent, SelectionMap};
pub use css::selectors::{InnerContent, SelectorQuery, SelectorQueryKind};
pub use xhtml::text_content::TextContent;
use css::selectors::Selectors;
use utils::reader::Reader;
use xhtml::parser::XHtmlParser;

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
