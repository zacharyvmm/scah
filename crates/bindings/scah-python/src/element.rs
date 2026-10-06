use pyo3::exceptions::{PyDeprecationWarning, PyValueError};
use pyo3::ffi::c_str;
use pyo3::types::PyDict;
use pyo3::{Bound, IntoPyObjectExt, prelude::*};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_core::{Attribute, ElementId, Store};
use std::sync::Arc;

#[gen_stub_pyclass]
#[pyclass(module = "scah", name = "Element")]
pub struct PyElement {
    pub(crate) store: Arc<Store<'static, 'static>>,
    pub(crate) id: ElementId,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyElement {
    #[getter]
    pub fn name(&self) -> Option<&str> {
        self.store.element(self.id).map(|e| e.name())
    }

    #[getter]
    pub fn class_name(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.class())
    }

    #[getter]
    pub fn id(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.id())
    }

    pub fn get_attribute(&self, key: String) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.attribute(&key))
    }

    #[getter]
    pub fn attributes<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let object = PyDict::new(py);
        let attributes = self.store.element(self.id).and_then(|e| e.attributes());

        if let Some(attrs) = attributes {
            for Attribute { key, value } in attrs {
                object.set_item(*key, *value)?
            }
        }
        Ok(object)
    }

    #[getter]
    pub fn inner_html(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.inner_html())
    }

    #[getter]
    pub fn raw_text(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.raw_text())
    }

    #[getter]
    pub fn text(&self) -> Option<&str> {
        self.store.element(self.id).and_then(|e| e.text())
    }

    #[getter]
    pub fn text_content<'a>(&'a self, py: Python<'_>) -> PyResult<Option<&'a str>> {
        PyErr::warn(
            py,
            &py.get_type::<PyDeprecationWarning>(),
            c_str!("Element.text_content is deprecated; use Element.text"),
            1,
        )?;
        Ok(self.text())
    }

    pub fn get(&self, query: String) -> PyResult<Vec<PyElement>> {
        match self.store.child_results(self.id, &query) {
            None => Err(PyValueError::new_err(format!(
                "This Element does not have children selected with `{query}`"
            ))),
            Some(children) => Ok(children
                .iter()
                .map(|&id| PyElement {
                    store: self.store.clone(),
                    id,
                })
                .collect()),
        }
    }

    pub fn keys(&self) -> Vec<&'static str> {
        vec![
            "name",
            "id",
            "class",
            "attributes",
            "inner_html",
            "raw_text",
            "text",
            "text_content",
        ]
    }

    pub fn __getitem__<'a>(&'a self, py: Python<'a>, key: &str) -> PyResult<Bound<'a, PyAny>> {
        match key {
            "name" => self.name().into_bound_py_any(py),
            "id" => self.id().into_bound_py_any(py),
            "class" => self.class_name().into_bound_py_any(py),
            "attributes" => self.attributes(py).and_then(|a| a.into_bound_py_any(py)),
            "inner_html" => self.inner_html().into_bound_py_any(py),
            "raw_text" => self.raw_text().into_bound_py_any(py),
            "text" => self.text().into_bound_py_any(py),
            "text_content" => self.text_content(py)?.into_bound_py_any(py),
            _ => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
    }
}

#[gen_stub_pyclass]
#[pyclass(module = "scah", name = "Store")]
pub(crate) struct PyStore {
    pub(crate) store: Arc<Store<'static, 'static>>,
    pub(crate) _html: Arc<String>,
    pub(crate) _query_tapes: Vec<Arc<Vec<u8>>>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStore {
    /// Results of the first query whose selector is `query`.
    fn get(&self, query: String) -> Option<Vec<PyElement>> {
        self.store.results(&query).map(|ids| self.elements(ids))
    }

    /// Results of the query at `index` in the list given to `parse`.
    fn query(&self, index: usize) -> Option<Vec<PyElement>> {
        self.store
            .query_results(index)
            .map(|ids| self.elements(ids))
    }

    fn __len__(&self) -> usize {
        self.store.len()
    }
}

impl PyStore {
    fn elements(&self, ids: &[ElementId]) -> Vec<PyElement> {
        ids.iter()
            .map(|&id| PyElement {
                store: self.store.clone(),
                id,
            })
            .collect()
    }
}
