use crate::arrow::{PyArrowTable, arrow_error};
use pyo3::exceptions::{PyDeprecationWarning, PyValueError};
use pyo3::ffi::c_str;
use pyo3::types::PyDict;
use pyo3::{Bound, IntoPyObjectExt, prelude::*};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_core::{Attribute, ElementId, ElementRef};
use scah_ffi::OwnedStore;
use scah_ffi::arrow::ArrowTable;
use std::sync::Arc;

#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "Element")]
pub struct PyElement {
    pub(crate) store: Arc<OwnedStore>,
    pub(crate) id: ElementId,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyElement {
    #[getter]
    pub fn name(&self) -> Option<&str> {
        self.element().map(|e| e.name())
    }

    #[getter]
    pub fn class_name(&self) -> Option<&str> {
        self.element().and_then(|e| e.class())
    }

    #[getter]
    pub fn id(&self) -> Option<&str> {
        self.element().and_then(|e| e.id())
    }

    pub fn get_attribute(&self, key: String) -> Option<&str> {
        self.element().and_then(|e| e.attribute(&key))
    }

    #[getter]
    pub fn attributes<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let object = PyDict::new(py);
        let attributes = self.element().and_then(|e| e.attributes());

        if let Some(attrs) = attributes {
            for Attribute { key, value } in attrs {
                object.set_item(key, value)?
            }
        }
        Ok(object)
    }

    #[getter]
    pub fn inner_html(&self) -> Option<&str> {
        self.element().and_then(|e| e.inner_html())
    }

    #[getter]
    pub fn raw_text(&self) -> Option<&str> {
        self.element().and_then(|e| e.raw_text())
    }

    #[getter]
    pub fn text(&self) -> Option<&str> {
        self.element().and_then(|e| e.text())
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
        match self.store.store().child_results(self.id, &query) {
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

impl PyElement {
    fn element(&self) -> Option<ElementRef<'_, '_, '_>> {
        self.store.store().element(self.id)
    }
}

#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "Store")]
pub(crate) struct PyStore {
    pub(crate) store: Arc<OwnedStore>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStore {
    /// Results of the first query whose selector is `query`.
    fn get(&self, query: String) -> Option<Vec<PyElement>> {
        self.store
            .store()
            .results(&query)
            .map(|ids| self.elements(ids))
    }

    /// Results of the query at `index` in the list given to `parse`.
    fn query(&self, index: usize) -> Option<Vec<PyElement>> {
        self.store
            .store()
            .query_results(index)
            .map(|ids| self.elements(ids))
    }

    fn __len__(&self) -> usize {
        self.store.store().len()
    }

    /// Lay out matched elements as Arrow columns.
    ///
    /// Rows are the elements matched by top-level selector `selector`, or,
    /// with `parent`, the elements matched by child selector `selector` under
    /// each element matched by top-level selector `parent`.
    ///
    /// Columns: `index` (uint32 element id), `parent` (uint32 parent id, only
    /// with `parent`), `tag`, `inner_html`, `raw_text`, `text`, then one per
    /// name in `attributes` (`id` and `class` included). All but `index`,
    /// `parent`, and `tag` are null when not saved or absent. Strings are
    /// `string_view` columns that borrow the store's memory instead of
    /// copying it.
    #[pyo3(signature = (selector, *, attributes=None, parent=None))]
    fn to_arrow(
        &self,
        selector: &str,
        attributes: Option<Vec<String>>,
        parent: Option<&str>,
    ) -> PyResult<PyArrowTable> {
        let attributes = attributes.unwrap_or_default();
        let attributes: Vec<&str> = attributes.iter().map(String::as_str).collect();
        ArrowTable::build(self.store.clone(), selector, parent, &attributes)
            .map(PyArrowTable::new)
            .map_err(arrow_error)
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
