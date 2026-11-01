use crate::polars::SectionSpec;
use crate::save::PySave;
use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_core::QuerySectionId;
use scah_core::lazy::{LazyQuery, LazyQueryBuilder};
use scah_ffi::OwnedQuery;

/// A query being built. Add child queries with `all`, `first`, or `then`,
/// then call `build`. `save` defaults to `Save.none()`.
#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "QueryBuilder")]
pub struct PyQueryBuilder {
    builder: LazyQueryBuilder<String>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryBuilder {
    /// Add a child query that keeps every match of `selector` inside each
    /// match of the last selector.
    #[pyo3(signature = (selector, save=None))]
    fn all(
        mut slf: PyRefMut<'_, Self>,
        selector: String,
        save: Option<PySave>,
    ) -> PyRefMut<'_, Self> {
        slf.builder.all_mut(selector, PySave::or_none(save));
        slf
    }
    /// Add a child query that keeps the first match of `selector` inside each
    /// match of the last selector.
    #[pyo3(signature = (selector, save=None))]
    fn first(
        mut slf: PyRefMut<'_, Self>,
        selector: String,
        save: Option<PySave>,
    ) -> PyRefMut<'_, Self> {
        slf.builder.first_mut(selector, PySave::or_none(save));
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
#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "QueryFactory")]
#[derive(Clone)]
pub struct PyQueryFactory {}

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryFactory {
    /// A child query that keeps every match of `selector`.
    #[pyo3(signature = (selector, save=None))]
    fn all(&self, selector: String, save: Option<PySave>) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, PySave::or_none(save)),
        }
    }

    /// A child query that keeps the first match of `selector`.
    #[pyo3(signature = (selector, save=None))]
    fn first(&self, selector: String, save: Option<PySave>) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, PySave::or_none(save)),
        }
    }
}

/// A built query, ready to pass to `parse`. It can be reused across documents.
#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "CompiledQuery")]
#[derive(Clone)]
pub struct PyQuery {
    pub(super) query: OwnedQuery,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyQuery {
    /// The root selector and the number of child queries, e.g.
    /// `CompiledQuery('main > section', 2 child queries)`.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let sections = &self.query.query().queries;
        let selector = PyString::new(py, sections[0].source).repr()?;
        Ok(match sections.len() - 1 {
            0 => format!("CompiledQuery({selector})"),
            1 => format!("CompiledQuery({selector}, 1 child query)"),
            n => format!("CompiledQuery({selector}, {n} child queries)"),
        })
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
///
/// `save` says what to keep for each match. It defaults to `Save.none()`: the
/// element's name and attributes.
#[gen_stub_pyclass(module = "scah._scah")]
#[pyclass(module = "scah", name = "Query")]
pub struct PyQueryStatic;

#[gen_stub_pymethods]
#[pymethods]
impl PyQueryStatic {
    #[staticmethod]
    /// Start a query that keeps every element matching `selector`.
    #[pyo3(signature = (selector, save=None))]
    pub fn all(selector: String, save: Option<PySave>) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, PySave::or_none(save)),
        }
    }

    #[staticmethod]
    /// Start a query that keeps the first element matching `selector`.
    #[pyo3(signature = (selector, save=None))]
    pub fn first(selector: String, save: Option<PySave>) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, PySave::or_none(save)),
        }
    }
}
