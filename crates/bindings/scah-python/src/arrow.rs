use pyo3::exceptions::{PyOverflowError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyCapsule;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_ffi::arrow::{ArrowArray, ArrowArrayStream, ArrowError, ArrowSchema, ArrowTable};
use std::ffi::CStr;
use std::sync::Arc;

/// Query results as Arrow columns.
///
/// Implements the Arrow PyCapsule Interface, so pyarrow, polars, DuckDB, and
/// other Arrow libraries can import it without copying strings, e.g.
/// `pyarrow.record_batch(table)` or `polars.DataFrame(table)`. Imported data
/// stays valid after the store and this table are gone.
#[gen_stub_pyclass]
#[pyclass(module = "scah", name = "ArrowTable", frozen)]
pub struct PyArrowTable {
    table: Arc<ArrowTable>,
}

impl PyArrowTable {
    pub(crate) fn new(table: ArrowTable) -> Self {
        Self {
            table: Arc::new(table),
        }
    }
}

pub(crate) fn arrow_error(err: ArrowError) -> PyErr {
    match err {
        ArrowError::TooLarge(_) => PyOverflowError::new_err(err.to_string()),
        _ => PyValueError::new_err(err.to_string()),
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyArrowTable {
    /// The number of rows.
    #[getter]
    fn num_rows(&self) -> usize {
        self.table.num_rows()
    }

    /// The column names, in order.
    #[getter]
    fn column_names(&self) -> Vec<&str> {
        self.table.column_names().collect()
    }

    fn __len__(&self) -> usize {
        self.table.num_rows()
    }

    /// Export the schema as an `arrow_schema` PyCapsule.
    #[gen_stub(override_return_type(type_repr = "typing.Any", imports = ("typing")))]
    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        capsule(py, self.table.export_schema(), c"arrow_schema")
    }

    /// Export the table as a record batch: a pair of `arrow_schema` and
    /// `arrow_array` PyCapsules. `requested_schema` is ignored.
    #[pyo3(signature = (requested_schema=None))]
    #[gen_stub(override_return_type(
        type_repr = "tuple[typing.Any, typing.Any]",
        imports = ("typing")
    ))]
    fn __arrow_c_array__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<(Bound<'py, PyCapsule>, Bound<'py, PyCapsule>)> {
        let _ = requested_schema;
        let (schema, array) = self.table.export();
        Ok((
            capsule(py, schema, c"arrow_schema")?,
            capsule(py, array, c"arrow_array")?,
        ))
    }

    /// Export the table as an `arrow_array_stream` PyCapsule yielding one
    /// record batch. `requested_schema` is ignored.
    #[pyo3(signature = (requested_schema=None))]
    #[gen_stub(override_return_type(type_repr = "typing.Any", imports = ("typing")))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        capsule(py, self.table.export_stream(), c"arrow_array_stream")
    }
}

/// An exported Arrow struct whose release callback may run on any thread.
trait Export {
    /// Release the struct unless a consumer moved it out (or released it).
    fn release_if_live(&mut self);
}

macro_rules! impl_export {
    ($($ty:ty),*) => {$(
        impl Export for $ty {
            fn release_if_live(&mut self) {
                if let Some(release) = self.release {
                    // SAFETY: a non-NULL release callback marks a live
                    // struct, which is released exactly once here.
                    unsafe { release(self) };
                }
            }
        }
    )*};
}

impl_export!(ArrowSchema, ArrowArray, ArrowArrayStream);

/// Capsule contents: the struct itself, so consumers find it at the capsule
/// pointer.
#[repr(transparent)]
struct Exported<T>(T);

// SAFETY: only structs exported by `scah_ffi::arrow` are wrapped. Their
// private data holds an `Arc<ArrowTable>` (Send + Sync) and owned vectors of
// pointers into immutable buffers, so they may be released on any thread.
unsafe impl<T: Export> Send for Exported<T> {}

/// Wrap an exported struct in a capsule named `name` whose destructor
/// releases the struct unless a consumer moved it out.
fn capsule<'py, T: Export + 'static>(
    py: Python<'py>,
    value: T,
    name: &CStr,
) -> PyResult<Bound<'py, PyCapsule>> {
    PyCapsule::new_with_destructor(py, Exported(value), Some(name.to_owned()), |value, _| {
        let Exported(mut value) = value;
        value.release_if_live();
    })
}
