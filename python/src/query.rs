use crate::save::PySave;
use ::scah::lazy::{LazyQuery, LazyQueryBuilder};
use ::scah::{QueryBuilder, Save, SelectionKind};
use pyo3::prelude::*;
use scah::Query;

#[pyclass]
pub struct PyQueryBuilder {
    builder: LazyQueryBuilder<String>,
}

#[pymethods]
impl PyQueryBuilder {
    fn all(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        slf.builder.all_mut(selector, save.save);
        slf
    }
    fn first(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        slf.builder.first_mut(selector, save.save);
        slf
    }

    fn then<'a>(
        mut slf: PyRefMut<'a, Self>,
        callback: Bound<'a, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let factory = PyQueryFactory {};
        let result = callback.call1((factory,))?;
        let builders: Vec<PyRef<PyQueryBuilder>> = result.extract()?;
        let children = builders.iter().map(|b| b.builder.clone());

        let current_index = slf.builder.len() - 1;
        for child in children {
            slf.builder.append(current_index, child);
        }

        Ok(slf)
    }

    fn build(&self) -> PyQuery {
        let (tape, query) = unsafe { self.builder.clone().to_query() };
        PyQuery { tape, query }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyQueryFactory {}

#[pymethods]
impl PyQueryFactory {
    fn all(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, save.save),
        }
    }

    fn first(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, save.save),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyQuery {
    tape: String,
    pub(super) query: Query<'static>,
}

#[pymethods]
impl PyQuery {
    fn __repr__(&self) -> String {
        format!("PyQuery(tape={:?}, query={:?})", self.tape, self.query)
    }
}

#[pyclass(name = "Query")]
pub struct PyQueryStatic;

#[pymethods]
impl PyQueryStatic {
    #[staticmethod]
    pub fn all(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::all(selector, save.save),
        }
    }

    #[staticmethod]
    pub fn first(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            builder: LazyQuery::first(selector, save.save),
        }
    }
}
