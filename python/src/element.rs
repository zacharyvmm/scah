use pyo3::prelude::*;
use pyo3::types::{PySlice, PyMemoryView, PyList};
use pyo3::exceptions::PyTypeError;

use std::ops::Range;

use super::view::SharedString;

type StrRange = Range<usize>;
type OptionalStrRange = Option<StrRange>;

pub struct Attribute {
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
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) children: Vec<(StrRange, Vec<usize>)>,

    pub(crate) inner_html: OptionalStrRange,
    pub(crate) text_content: OptionalStrRange,
}

fn slice_buffer<'py>(
    element: &PyElement, 
    py: Python<'py>, 
    range: Range<usize>
) -> PyResult<Bound<'py, PyMemoryView>> {
    let slice = PySlice::new(py, range.start as isize, range.end as isize, 1);
    
    let sliced_view = element.base.bind(py).get_item(slice)?;
    sliced_view.downcast_into::<PyMemoryView>()
        .map_err(|_| PyTypeError::new_err("Result of slice was not a memoryview"))
}

#[pymethods]
impl PyElement {
    #[getter]
    fn name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        slice_buffer(self, py, self.name.clone())
    }

    #[getter]
    fn class<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.class {
            slice_buffer(self, py, range.clone())
        }else {
            Err(PyTypeError::new_err("`class` does not exist"))
        }
    }

    #[getter]
    fn id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.id {
            slice_buffer(self, py, range.clone())
        }else {
            Err(PyTypeError::new_err("`id` does not exist"))
        }
    }

    #[getter]
    fn inner_html<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.inner_html {
            slice_buffer(self, py, range.clone())
        }else {
            Err(PyTypeError::new_err("`inner_html` does not exist"))
        }
    }

    #[getter]
    fn source<'py>(&self, py: Python<'py>) -> Py<PyMemoryView> {
        self.text_content_tape.clone_ref(py)
    }

    #[getter]
    fn text_content<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        if let Some(range) = &self.text_content {
            let slice = PySlice::new(py, range.start as isize, range.end as isize, 1);
            
            let sliced_view = self.text_content_tape.bind(py).get_item(slice)?;
            sliced_view.downcast_into::<PyMemoryView>()
                .map_err(|_| PyTypeError::new_err("Result of slice was not a memoryview"))
        }else {
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
}