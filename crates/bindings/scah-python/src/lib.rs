use pyo3::prelude::*;
use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};

use std::sync::Arc;

mod element;
mod query;
mod save;

use crate::query::{PyQuery, PyQueryBuilder, PyQueryFactory, PyQueryStatic};
use crate::save::PySave;
use element::{PyElement, PyStore};
use scah_ffi::OwnedStore;

#[gen_stub_pyfunction]
#[pyfunction]
fn parse(html: String, queries: Vec<PyRef<PyQuery>>) -> PyResult<PyStore> {
    let queries: Vec<_> = queries.iter().map(|q| &q.query).collect();
    let store = match OwnedStore::parse(html, &queries) {
        Ok(store) => store,
        Err(scah_core::ParseError::EmptyQueries) => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "parse requires at least one query",
            ));
        }
        Err(scah_core::ParseError::MaximumDepthExceeded) => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "HTML nesting depth exceeds the maximum supported depth",
            ));
        }
        Err(scah_core::ParseError::TextCaptureRequired) => {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "internal error: unexpected TextCaptureRequired from parse",
            ));
        }
    };

    Ok(PyStore {
        store: Arc::new(store),
    })
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

define_stub_info_gatherer!(stub_info);
