use pyo3::exceptions::PyTypeError;
use pyo3::types::{PyBytes, PyDict, PyList, PyMemoryView, PySlice, PyString};
use pyo3::{Bound, BoundObject, prelude::*};

use std::ops::{Index, Range};

#[pyclass(name = "Attribute")]
pub struct PyAttribute {
    pub(crate) key: Py<PyMemoryView>,
    pub(crate) value: Option<Py<PyMemoryView>>,
}

#[pyclass(name = "Element")]
pub struct PyElement {
    pub(crate) name: Py<PyMemoryView>,
    pub(crate) class: Option<Py<PyMemoryView>>,
    pub(crate) id: Option<Py<PyMemoryView>>,
    pub(crate) attributes: Py<PyList>,

    pub(crate) inner_html: Option<Py<PyMemoryView>>,
    pub(crate) text_content: Option<Py<PyMemoryView>>,

    pub(crate) children: Vec<(Py<PyMemoryView>, Vec<usize>)>,
}

pub trait PyMemoryViewExt<'py> {
    fn slice_range(&self, range: std::ops::Range<usize>) -> PyResult<Bound<'py, PyMemoryView>>;
}

impl<'py> PyMemoryViewExt<'py> for Bound<'py, PyMemoryView> {
    fn slice_range(&self, range: std::ops::Range<usize>) -> PyResult<Bound<'py, PyMemoryView>> {
        let slice = PySlice::new(self.py(), range.start as isize, range.end as isize, 1);
        let sliced_view = self.get_item(slice)?;
        sliced_view
            .downcast_into::<PyMemoryView>()
            .map_err(|e| e.into())
    }
}

fn get_string_from_buffer(py: Python<'_>, buffer: &Py<PyMemoryView>) -> PyResult<String> {
    let pystr = PyString::from_encoded_object(&buffer.as_any().bind(py), Some(&c"utf-8"), None)?;
    Ok(pystr.to_string_lossy().into_owned())
}

#[pymethods]
impl PyAttribute {
    #[getter]
    fn key<'py>(&self, py: Python<'py>) -> Py<PyMemoryView> {
        self.key.clone_ref(py)
    }

    #[getter]
    fn value<'py>(&self, py: Python<'py>) -> Option<Py<PyMemoryView>> {
        self.value.as_ref().and_then(|v| Some(v.clone_ref(py)))
    }

    fn __repr__(&self) -> PyResult<String> {
        Python::with_gil(|py| {
            let key = get_string_from_buffer(py, &self.key)?;
            let value = match &self.value {
                Some(v) => format!("{:?}", get_string_from_buffer(py, v)?),
                None => "None".to_string(),
            };
            Ok(format!("Attribute(key={:?}, value={})", key, value))
        })
    }
}

#[pymethods]
impl PyElement {
    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> Py<PyMemoryView> {
        self.name.clone_ref(py)
    }

    #[getter]
    fn class<'py>(&self, py: Python<'py>) -> Option<Py<PyMemoryView>> {
        self.class.as_ref().and_then(|v| Some(v.clone_ref(py)))
    }

    #[getter]
    fn id<'py>(&self, py: Python<'py>) -> Option<Py<PyMemoryView>> {
        self.id.as_ref().and_then(|v| Some(v.clone_ref(py)))
    }

    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> Py<PyList> {
        self.attributes.clone_ref(py)
    }

    #[getter]
    fn inner_html<'py>(&self, py: Python<'py>) -> Option<Py<PyMemoryView>> {
        self.inner_html.as_ref().and_then(|v| Some(v.clone_ref(py)))
    }

    #[getter]
    fn text_content<'py>(&self, py: Python<'py>) -> Option<Py<PyMemoryView>> {
        self.text_content
            .as_ref()
            .and_then(|v| Some(v.clone_ref(py)))
    }

    fn __repr__(&self) -> PyResult<String> {
        Python::with_gil(|py| {
            let name = get_string_from_buffer(py, &self.name)?;
            let id = match &self.id {
                Some(v) => format!("{:?}", get_string_from_buffer(py, v)?),
                None => "None".to_string(),
            };
            let class = match &self.class {
                Some(v) => format!("{:?}", get_string_from_buffer(py, v)?),
                None => "None".to_string(),
            };
            Ok(format!(
                "Element(name={:?}, id={}, class={})",
                name, id, class
            ))
        })
    }
}

#[pyclass(name = "Store")]
pub(crate) struct PyStore {
    pub(crate) elements: Py<PyList>,
    pub(crate) text_content: Py<PyMemoryView>,
}

#[pymethods]
impl PyStore {
    #[getter]
    fn elements<'py>(&self, py: Python<'py>) -> Py<PyList> {
        self.elements.clone_ref(py)
    }

    #[getter]
    fn _text_content<'py>(&self, py: Python<'py>) -> Py<PyMemoryView> {
        self.text_content.clone_ref(py)
    }

    fn children_of<'py>(
        &self,
        py: Python<'py>,
        element: &PyElement,
    ) -> PyResult<Bound<'py, PyDict>> {
        let mut dict = PyDict::new(py);
        for (query, children) in &element.children {
            let string = PyString::from_encoded_object(query.bind(py), Some(&c"utf-8"), None)?;
            if children.len() > 1 {
                let mut list = PyList::empty(py);
                for i in children {
                    let item = self.elements.bind(py).get_item(*i)?;
                    list.append(item).expect("Was not able to append to list");
                }
                dict.set_item(string, list)
                    .expect("Was not able to add entry in Dictionary");
            } else {
                let item = self.elements.bind(py).get_item(children[0])?;
                dict.set_item(string, item)
                    .expect("Was not able to add entry in Dictionary");
            }
        }

        Ok(dict)
    }

    fn __repr__(&self) -> PyResult<String> {
        Python::with_gil(|py| {
            let elements_len = self.elements.bind(py).len();
            let text_content_len = self.text_content.bind(py).len()?;
            Ok(format!(
                "Store(elements_count={}, text_content_length={})",
                elements_len, text_content_len
            ))
        })
    }
}
