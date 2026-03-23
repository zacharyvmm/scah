use ::scah::{Query, QueryBuilder, QueryMultiplexer, Reader, Store, XHtmlParser};
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyMemoryView};

mod element;
mod query;
mod save;

use crate::query::{PyQuery, PyQueryBuilder, PyQueryFactory, PyQueryStatic};
use crate::save::PySave;
use element::{PyElement, PyStore};

#[pyfunction]
fn parse<'py>(py: Python<'py>, html: String, queries: Bound<'py, PyAny>) -> PyResult<PyStore> {
    let len = html.as_bytes().len();
    let html_bytes = unsafe { std::slice::from_raw_parts(html.as_ptr(), len) };

    let mut py_queries: Vec<PyQuery> = Vec::new();

    if let Ok(single_query) = queries.extract::<PyQuery>() {
        py_queries.push(single_query);
    } else if let Ok(list) = Bound::cast::<PyList>(&queries) {
        for item in list {
            py_queries.push(item.extract::<PyQuery>()?);
        }
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Query value must be a Query object or a list of Query objects",
        ));
    }

    let queries_rs = py_queries
        .iter()
        .map(|q| q.query.clone())
        .collect::<Box<[Query]>>();

    let slice = unsafe { std::slice::from_raw_parts(queries_rs.as_ptr(), queries_rs.len()) };
    let selectors = QueryMultiplexer::new(slice);
    let mut parser = XHtmlParser::with_capacity(selectors, html_bytes.len());

    let mut reader = Reader::from_bytes(html_bytes);
    while parser.next(&mut reader) {}

    let store = std::sync::Arc::new(parser.matches());

    Ok(PyStore { store, _html: html })
}

#[pymodule]
fn scah(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_class::<PySave>()?;
    m.add_class::<PyQuery>()?;
    m.add_class::<PyQueryBuilder>()?;
    m.add_class::<PyQueryStatic>()?;
    m.add_class::<PyQueryFactory>()?;
    m.add_class::<PyElement>()?;
    m.add_class::<PyStore>()?;
    Ok(())
}
