use super::SharedString;
use pyo3::exceptions::PyTypeError;
use pyo3::types::{PyBytes, PyDict, PyList, PyMemoryView, PySlice, PyString};
use pyo3::{Bound, BoundObject, prelude::*};
use std::ops::{Index, Range};

type StrRange = Range<usize>;
type OptionalStrRange = Option<StrRange>;

#[pyclass(name = "Element")]
pub struct PyAttribute {
    pub(crate) base: Py<PyMemoryView>,
    pub(crate) key: StrRange,
    pub(crate) value: OptionalStrRange,
}

#[pyclass(name = "Element")]
pub struct PyElement {
    pub(crate) base: Py<PyMemoryView>,
    pub(crate) text_content_tape: Py<PyMemoryView>,

    pub(crate) name: StrRange,
    pub(crate) class: OptionalStrRange,
    pub(crate) id: OptionalStrRange,
    pub(crate) attributes: Py<PyList>, // list of PyAttributes

    pub(crate) inner_html: OptionalStrRange,
    pub(crate) text_content: OptionalStrRange,

    pub(crate) children: Vec<(StrRange, Vec<usize>)>,
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

fn get_string_from_buffer(
    py: Python<'_>,
    base: &Py<PyMemoryView>,
    range: &Range<usize>,
) -> PyResult<String> {
    let mv = base.bind(py).slice_range(range.clone())?;
    let pystr = PyString::from_encoded_object(&mv, Some(&c"utf-8"), None)?;
    Ok(pystr.to_string_lossy().into_owned())
}

#[pymethods]
impl PyAttribute {
    #[getter]
    fn key<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        self.base.bind(py).slice_range(self.key.clone())
    }

    #[getter]
    fn value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        match &self.value {
            Some(range) => self.base.bind(py).slice_range(range.clone()),
            None => Err(PyTypeError::new_err("`value` does not exist")),
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Python::with_gil(|py| {
            let key = get_string_from_buffer(py, &self.base, &self.key)?;
            let value = match &self.value {
                Some(range) => format!("{:?}", get_string_from_buffer(py, &self.base, range)?),
                None => "None".to_string(),
            };
            Ok(format!("Attribute(key={:?}, value={})", key, value))
        })
    }
}

#[pymethods]
impl PyElement {
    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        self.base.bind(py).slice_range(self.name.clone())
    }

    #[getter]
    fn className<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        match &self.class {
            Some(range) => self.base.bind(py).slice_range(range.clone()),
            None => Err(PyTypeError::new_err("`className` does not exist")),
        }
    }

    #[getter]
    fn id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        match &self.id {
            Some(range) => self.base.bind(py).slice_range(range.clone()),
            None => Err(PyTypeError::new_err("`id` does not exist")),
        }
    }

    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> Py<PyList> {
        self.attributes.clone_ref(py)
    }

    #[getter]
    fn inner_html<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        match &self.inner_html {
            Some(range) => self.base.bind(py).slice_range(range.clone()),
            None => Err(PyTypeError::new_err("`id` does not exist")),
        }
    }

    #[getter]
    fn text_content<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        match &self.text_content {
            Some(range) => self.text_content_tape.bind(py).slice_range(range.clone()),
            None => Err(PyTypeError::new_err("`id` does not exist")),
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Python::with_gil(|py| {
            let name = get_string_from_buffer(py, &self.base, &self.name)?;
            let id = match &self.id {
                Some(range) => format!("{:?}", get_string_from_buffer(py, &self.base, range)?),
                None => "None".to_string(),
            };
            let class = match &self.class {
                Some(range) => format!("{:?}", get_string_from_buffer(py, &self.base, range)?),
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
    pub(crate) text_content: SharedString,
}

#[pymethods]
impl PyStore {
    #[getter]
    fn elements<'py>(&self, py: Python<'py>) -> Py<PyList> {
        self.elements.clone_ref(py)
    }

    #[getter]
    fn _text_content<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        self.text_content.as_view(py)
    }

    fn children_of<'py>(
        &self,
        py: Python<'py>,
        element: &PyElement,
    ) -> PyResult<Bound<'py, PyDict>> {
        let mut dict = PyDict::new(py);
        for (query, children) in &element.children {
            let mv = element.base.bind(py).slice_range(query.clone())?;
            let string = PyString::from_encoded_object(&mv, Some(&c"utf-8"), None)?;
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
            let text_content_len = self.text_content.inner.as_ref().len();
            Ok(format!(
                "Store(elements_count={}, text_content_length={})",
                elements_len, text_content_len
            ))
        })
    }
}
