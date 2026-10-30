use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};

use std::sync::Arc;

mod arrow;
mod element;
mod query;
mod save;

use crate::query::{PyQuery, PyQueryBuilder, PyQueryFactory, PyQueryStatic};
use crate::save::PySave;
use element::{PyElement, PyStore};
use scah_core::ParseError;
use scah_ffi::{OwnedQuery, OwnedStore};

#[gen_stub_pyfunction]
#[pyfunction]
fn parse(html: String, queries: Vec<PyRef<PyQuery>>) -> PyResult<PyStore> {
    let queries: Vec<&OwnedQuery> = queries.iter().map(|q| &q.query).collect();
    let store = OwnedStore::parse(html, &queries).map_err(parse_error)?;
    Ok(PyStore {
        store: Arc::new(store),
    })
}

/// The Python exception for a failed parse.
fn parse_error(err: ParseError) -> PyErr {
    match err {
        // Only `parse_without_text_capture` reports this.
        ParseError::TextCaptureRequired => {
            PyRuntimeError::new_err("internal error: unexpected TextCaptureRequired from parse")
        }
        err => PyValueError::new_err(err.to_string()),
    }
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
    m.add_class::<arrow::PyArrowTable>()?;
    Ok(())
}

define_stub_info_gatherer!(stub_info);
