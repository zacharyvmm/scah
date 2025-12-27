mod css;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;
use utils::Reader;
use xhtml::parser::XHtmlParser;

use crate::css::FsmManager;
use crate::store::RustStore;

pub use css::{Save, Selection, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, SelectionValue};
pub use xhtml::element::element::{Attribute, XHtmlElement};

pub fn parse<'html: 'query, 'query: 'html>(
    html: &'html str,
    queries: &'query Vec<Selection<'query>>,
) -> HashMap<&'query str, SelectionValue<'html, 'query>> {
    let selectors = FsmManager::<RustStore>::new(queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    return parser.matches().root.children;
}


#[cfg(feature = "python")]
#[pyo3::pymodule]
mod onego {
  use pyo3::prelude::*;

  /// Formats the sum of two numbers as string.
  #[pyfunction]
  fn parse(query: &str, html: &str) -> PyResult<String> {
    Ok("Hello World".to_string())
  }
}