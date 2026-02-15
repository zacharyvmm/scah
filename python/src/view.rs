use pyo3::ffi::{PyBUF_READ, PyMemoryView_FromMemory};
use pyo3::types::{PyByteArray, PyBytes, PyCapsule, PyMemoryView};
use pyo3::{CastIntoError, prelude::*};
use std::ffi::CStr;

pub fn string_buffer_to_memory_view(
    py: Python<'_>,
    input: Vec<u8>,
) -> PyResult<Bound<'_, PyMemoryView>> {
    let boxed_string = Box::new(input);
    let ptr = boxed_string.as_ptr() as *mut i8;

    //let len = boxed_string.len() as isize;
    let len = boxed_string.capacity() as isize;

    let name = unsafe { CStr::from_bytes_with_nul_unchecked(b"string_buffer\0") };
    let capsule = PyCapsule::new(py, boxed_string, Some(name.to_owned()))?;
    let object = unsafe { PyMemoryView_FromMemory(ptr, len, PyBUF_READ) };
    let res = unsafe { Bound::from_owned_ptr_or_err(py, object)?.cast_into::<PyMemoryView>() };
    res.map_err(|e| e.into())
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
