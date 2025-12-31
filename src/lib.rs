mod css;
mod store;
mod utils;
mod xhtml;

use std::collections::HashMap;
use utils::Reader;
use xhtml::parser::XHtmlParser;

use crate::css::FsmManager;
use crate::store::{RustStore, Store};

pub use css::{Query, QueryBuilder, Save, SelectionKind, SelectionPart};
pub use store::{Element, QueryError, SelectionValue};
pub use xhtml::element::element::{Attribute, XHtmlElement};

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

#[cfg(feature = "python")]
#[pyo3::pymodule]
mod onego {
    // import the real libs, so that useless rust only code, does come in
    use super::*;
    use crate::store::{PythonStore, Store};
    use pyo3::prelude::*;
    use pyo3::types::{PyDict, PyString};

    #[pyfunction]
    fn parse<'py>(
        py: Python<'py>,
        html: &str,
        queries: &Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let keys_list = queries.keys();
        //let items: Vec<PyObject> = keys_list.extract()?;
        // Assuming a single query
        let query_any = keys_list
            .get_item(0)
            .expect("Should have at least a single key");
        let query_string = query_any
            .cast::<PyString>()
            .expect("A key must be a string");
        let query = query_string.extract().unwrap();
        let queries = &[QueryBuilder::new(SelectionPart::new(
            query,
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))
        .build()];
        let store = PythonStore::new(py);
        let selectors = FsmManager::new(store, queries);
        let mut parser = XHtmlParser::new(selectors);

        let mut reader = Reader::new(html);
        while parser.next(&mut reader) {}

        let out_store = parser.matches();
        Ok(out_store.root)
    }
}
