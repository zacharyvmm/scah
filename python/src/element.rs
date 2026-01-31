use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyMemoryView, PySlice, PyString};

use std::ops::Range;

use super::view::SharedString;

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
    // TODO: I might make this part of Store
    // add generate the PyElement in a iterator
    // Probably more memory efficient too because
    // the gc would have to delete the unused element classes
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

fn slice_buffer<'py>(
    py: Python<'py>,
    view: &Py<PyMemoryView>,
    range: &Range<usize>,
) -> PyResult<Bound<'py, PyMemoryView>> {
    let slice = PySlice::new(py, range.start as isize, range.end as isize, 1);

    let sliced_view = view.bind(py).get_item(slice)?;
    sliced_view
        .downcast_into::<PyMemoryView>()
        .map_err(|_| PyTypeError::new_err("Result of slice was not a memoryview"))
}

#[pymethods]
impl PyAttribute {
    #[getter]
    fn key<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        slice_buffer(py, &self.base, &self.key)
    }

    #[getter]
    fn value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.value {
            slice_buffer(py, &self.base, range)
        } else {
            Err(PyTypeError::new_err("`value` does not exist"))
        }
    }
}

#[pymethods]
impl PyElement {
    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        slice_buffer(py, &self.base, &self.name)
    }

    #[getter]
    fn class<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.class {
            slice_buffer(py, &self.base, range)
        } else {
            Err(PyTypeError::new_err("`class` does not exist"))
        }
    }

    #[getter]
    fn id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.id {
            slice_buffer(py, &self.base, range)
        } else {
            Err(PyTypeError::new_err("`id` does not exist"))
        }
    }

    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> Py<PyList> {
        self.attributes.clone_ref(py)
    }

    #[getter]
    fn inner_html<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.inner_html {
            slice_buffer(py, &self.base, range)
        } else {
            Err(PyTypeError::new_err("`inner_html` does not exist"))
        }
    }

    #[getter]
    fn text_content<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.text_content {
            slice_buffer(py, &self.text_content_tape, range)
        } else {
            Err(PyTypeError::new_err("`text_content` does not exist"))
        }
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
    fn elements<'py>(&self, py: Python<'py>) -> PyResult<Py<PyList>> {
        Ok(self.elements.clone_ref(py))
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
            let mv = slice_buffer(py, &element.base, &query)?;
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
}
