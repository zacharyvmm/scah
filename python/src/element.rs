use pyo3::exceptions::PyValueError;
use pyo3::types::PyDict;
use pyo3::{Bound, IntoPyObjectExt, prelude::*};
use scah::{Attribute, ElementId, Store};
use std::sync::Arc;

#[pyclass(name = "Element")]
pub struct PyElement {
    pub(crate) store: Arc<Store<'static, 'static>>,
    pub(crate) id: ElementId,
}

#[pymethods]
impl PyElement {
    #[getter]
    pub fn name(&self) -> Option<&str> {
        self.store.elements.get(self.id.0).map(|e| e.name)
    }

    #[getter]
    pub fn class_name(&self) -> Option<&str> {
        self.store.elements.get(self.id.0).and_then(|e| e.class)
    }

    #[getter]
    pub fn id(&self) -> Option<&str> {
        self.store.elements.get(self.id.0).and_then(|e| e.id)
    }

    pub fn get_attribute(&self, key: String) -> Option<&str> {
        self.store
            .elements
            .get(self.id.0)
            .and_then(|e| e.attribute(&self.store, &key))
    }

    #[getter]
    pub fn attributes<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let mut object = PyDict::new(py);
        let attributes = self
            .store
            .elements
            .get(self.id.0)
            .and_then(|e| e.attributes(&self.store));

        if let Some(attrs) = attributes {
            for Attribute { key, value } in attrs {
                object.set_item(*key, *value)?
            }
        }
        Ok(object)
    }

    #[getter]
    pub fn inner_html(&self) -> Option<&str> {
        self.store
            .elements
            .get(self.id.0)
            .and_then(|e| e.inner_html)
    }

    #[getter]
    pub fn text_content(&self) -> Option<&str> {
        self.store
            .elements
            .get(self.id.0)
            .and_then(|e| e.text_content(&self.store))
    }

    pub fn children(&self, query: String) -> PyResult<Vec<PyElement>> {
        let element = self
            .store
            .elements
            .get(self.id.0)
            .expect("The Element ID should be valid");
        let children = element.get(&self.store, &query);
        match children {
            None => {
                return Err(PyValueError::new_err(format!(
                    "This Element does not have children selected with `{query}`"
                )));
            }
            Some(children) => Ok(children
                .map(|e| PyElement {
                    store: self.store.clone(),
                    id: unsafe { self.store.elements.index_of(e) },
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
            "text_content" => self.text_content().into_bound_py_any(py),
            _ => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
    }
}

#[pyclass(name = "Store")]
pub(crate) struct PyStore {
    pub(crate) store: Arc<Store<'static, 'static>>,
    pub(crate) _html: Arc<String>,
}

#[pymethods]
impl PyStore {
    fn get(&self, query: String) -> Option<Vec<PyElement>> {
        self.store.get(&query).map(|iter| {
            iter.map(|e| unsafe { self.store.elements.index_of(e) })
                .map(|i| PyElement {
                    store: self.store.clone(),
                    id: i,
                })
                .collect()
        })
    }

    #[getter]
    fn length(&self) -> i64 {
        self.store.elements.len() as i64
    }
}
