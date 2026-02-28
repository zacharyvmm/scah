use pyo3::ffi::{PyBUF_READ, PyMemoryView_FromMemory};
use pyo3::types::{PyByteArray, PyBytes, PyCapsule, PyMemoryView};
use pyo3::{CastIntoError, prelude::*};
use std::ffi::CStr;

#[pyclass]
pub(crate) struct SharedString {
    pub(crate) inner: Box<[u8]>,
}

#[pymethods]
impl SharedString {
    pub(crate) fn as_view<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyMemoryView>> {
        let ptr = self.inner.as_ptr() as *mut i8;
        let len = self.inner.len() as isize;

        let object = unsafe { PyMemoryView_FromMemory(ptr, len, PyBUF_READ) };

        let res = unsafe { Bound::from_owned_ptr_or_err(py, object)?.cast_into::<PyMemoryView>() };
        if res.is_err() {
            return Err(PyErr::fetch(py));
        }
        Ok(res.unwrap())
    }
}

pub fn get_memoryview_from_u8<'a>(
    py: Python<'a>,
    text: &'a [u8],
) -> PyResult<Bound<'a, PyMemoryView>> {
    // INSANELY unsafe, but I'll assume that the user doesn't mess with the underling string
    let object = unsafe {
        PyMemoryView_FromMemory(text.as_ptr() as *mut i8, text.len() as isize, PyBUF_READ)
    };
    let res = unsafe { Bound::from_owned_ptr_or_err(py, object)?.cast_into::<PyMemoryView>() };
    if res.is_err() {
        return Err(PyErr::fetch(py));
    }
    Ok(res.unwrap())
}

pub fn is_subslice<T>(
    parent_range: &std::ops::Range<*const T>,
    subslice_range: &std::ops::Range<*const T>,
) -> bool {
    parent_range == subslice_range
        || (parent_range.contains(&subslice_range.start)
            && parent_range.contains(&subslice_range.end))
}
