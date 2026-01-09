use ::onego::{FsmManager, Reader, XHtmlParser, QueryBuilder, SelectionPart, Store};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

mod selection;
mod element;
mod store;
mod save;
mod query;

use crate::store::PythonStore;
use crate::save::PySave;
use crate::query::{PyQuery, PyQueryBuilder, PyQueryStatic, PyQueryFactory};

#[pyfunction]
fn parse<'py>(
    py: Python<'py>,
    html: &str,
    queries: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    // queries can be a PyQuery OR a List[PyQuery]
    
    let mut py_queries: Vec<PyQuery> = Vec::new();

    if let Ok(single_query) = queries.extract::<PyQuery>() {
        py_queries.push(single_query);
    } else if let Ok(list) = queries.downcast::<PyList>() {
        for item in list {
            py_queries.push(item.extract::<PyQuery>()?);
        }
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Query value must be a Query object or a list of Query objects",
        ));
    }

    let mut built_queries = Vec::with_capacity(py_queries.len());

    // We need to keep the Queries alive, which means the Strings inside them must be alive.
    // The `py_queries` vector owns the `PyQuery` objects (clones).
    
    // We iterate through py_queries and build Rust Query objects borrowing from them.
    for py_query in &py_queries {
        if py_query.builder.is_empty() {
             continue; 
        }

        // Convert Vec<SelectionPart<String>> to Vec<SelectionPart<&str>>
        let parts_ref: Vec<SelectionPart<&str>> = py_query.builder.iter().map(|part| {
            SelectionPart {
                source: part.source.as_str(),
                kind: part.kind,
                parent: part.parent,
                children: part.children.clone(),
            }
        }).collect();
        
        // Reconstruct QueryBuilder for &str
        let builder = QueryBuilder::from_list(parts_ref);
        
        built_queries.push(builder.build());
    }
    
    let store = PythonStore::new(py);
    let selectors = FsmManager::new(store, &built_queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    let out_store = parser.matches();
    let root = out_store.root;
    let children_any = root.get_item("children").expect("Their should be a children feild on the root").expect("Thier should be a value associated with the field `children` of root");
    let children = children_any
        .cast::<PyDict>()
        .expect("the children field value of root is a dict").to_owned();
    Ok(children)
}

#[pymodule]
fn onego(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_class::<PySave>()?;
    m.add_class::<PyQuery>()?;
    m.add_class::<PyQueryBuilder>()?;
    m.add_class::<PyQueryStatic>()?;
    m.add_class::<PyQueryFactory>()?;
    Ok(())
}
