use crate::polars::SectionSpec;
use crate::save::PySave;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_core::QuerySectionId;
use scah_core::lazy::{LazyQuery, LazyQueryBuilder};
use scah_ffi::OwnedQuery;

/// A query being built. Add child queries with `all`, `first`, or `then`,
/// then call `build`.
#[gen_stub_pyclass]
#[pyclass]
pub struct PyQueryBuilder {
    builder: LazyQueryBuilder<String>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryBuilder {
    /// Add a child query that keeps every match of `selector` inside each
    /// match of the last selector.
    fn all(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        slf.builder.all_mut(selector, save.save);
        slf
    }
    /// Add a child query that keeps the first match of `selector` inside each
    /// match of the last selector.
    fn first(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        slf.builder.first_mut(selector, save.save);
        slf
    }

    /// Add the child queries that `callback(factory)` returns. They match inside
    /// each match of the last selector; build them with `factory.all()` and
    /// `factory.first()`.
    fn then<'a>(
        mut slf: PyRefMut<'a, Self>,
        callback: Bound<'a, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let factory = PyQueryFactory {};
        let result = callback.call1((factory,))?;
        let builders: Vec<PyRef<PyQueryBuilder>> = result.extract()?;
        let children = builders.iter().map(|b| b.builder.clone());

        let current_index = QuerySectionId(slf.builder.len() - 1);
        for child in children {
            slf.builder.append(current_index, child);
        }

        Ok(slf)
    }

    /// Finish the query, ready to pass to `parse`.
    ///
    /// Raises `ValueError` if a selector is invalid or uses syntax scah cannot
    /// evaluate in one pass, such as `:has()` or `:last-child`.
    fn build(&self) -> PyResult<PyQuery> {
        let query = OwnedQuery::build(self.builder.clone())
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
        Ok(PyQuery { query })
    }
}

/// Builds child queries inside a `then` callback.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct PyQueryFactory {}

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryFactory {
    /// A child query that keeps every match of `selector`.
    fn all(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, save.save),
        }
    }

    /// A child query that keeps the first match of `selector`.
    fn first(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, save.save),
        }
    }
}

/// A built query, ready to pass to `parse`. It can be reused across documents.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct PyQuery {
    pub(super) query: OwnedQuery,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyQuery {
    fn __repr__(&self) -> String {
        format!("PyQuery(query={:?})", self.query)
    }

    /// The query's sections, root first, as dicts: how `scah.polars`
    /// passes the query to its plugin.
    #[gen_stub(override_return_type(
        type_repr = "builtins.list[builtins.dict[builtins.str, typing.Any]]",
        imports = ("builtins", "typing")
    ))]
    fn _sections(&self) -> Vec<SectionSpec> {
        self.query
            .query()
            .queries
            .iter()
            .map(|section| {
                SectionSpec::new(section.source, section.kind, section.parent, section.save)
            })
            .collect()
    }
}

/// Entry point for building queries.
#[gen_stub_pyclass]
#[pyclass(name = "Query")]
pub struct PyQueryStatic;

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryStatic {
    #[staticmethod]
    /// Start a query that keeps every element matching `selector`.
    pub fn all(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, save.save),
        }
    }

    #[staticmethod]
    /// Start a query that keeps the first element matching `selector`.
    pub fn first(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, save.save),
        }
    }
}
