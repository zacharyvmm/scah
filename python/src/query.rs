use crate::save::PySave;
use ::scah::{QueryBuilder, SelectionKind, SelectionPart};
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct PyQueryBuilder {
    pub inner: QueryBuilder<'static, String>,
}

#[pymethods]
impl PyQueryBuilder {
    fn all(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        let part = SelectionPart::new(selector, SelectionKind::All(save.save));
        let len = slf.inner.list.len();
        let mut p = part;
        p.parent = Some(len - 1);
        slf.inner.list.push(p);

        slf
    }

    fn first(mut slf: PyRefMut<'_, Self>, selector: String, save: PySave) -> PyRefMut<'_, Self> {
        let part = SelectionPart::new(selector, SelectionKind::First(save.save));
        let len = slf.inner.list.len();
        let mut p = part;
        p.parent = Some(len - 1);
        slf.inner.list.push(p);

        slf
    }

    fn then<'a>(
        mut slf: PyRefMut<'a, Self>,
        callback: Bound<'a, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let factory = PyQueryFactory {};
        let result = callback.call1((factory,))?;
        let builders: Vec<PyQueryBuilder> = result.extract()?;

        let parent_index = slf.inner.list.len() - 1;
        let mut offset = 0;

        for builder in builders {
            for (_i, mut part) in builder.inner.list.into_iter().enumerate() {
                part.parent = match part.parent {
                    None => {
                        offset += 1;
                        Some(parent_index)
                    }
                    Some(p) => Some(p + slf.inner.list.len() + offset),
                };
                slf.inner.list.push(part);
            }
        }
        Ok(slf)
    }

    fn build(&self) -> PyQuery {
        PyQuery {
            builder: self.inner.list.clone(),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyQueryFactory {}

#[pymethods]
impl PyQueryFactory {
    fn all(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            inner: QueryBuilder::new(SelectionPart::new(selector, SelectionKind::All(save.save))),
        }
    }

    fn first(&self, selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            inner: QueryBuilder::new(SelectionPart::new(
                selector,
                SelectionKind::First(save.save),
            )),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyQuery {
    pub builder: Vec<SelectionPart<String>>,
}

#[pyclass(name = "Query")]
pub struct PyQueryStatic;

#[pymethods]
impl PyQueryStatic {
    #[staticmethod]
    pub fn all(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            inner: QueryBuilder::new(SelectionPart::new(selector, SelectionKind::All(save.save))),
        }
    }

    #[staticmethod]
    pub fn first(selector: String, save: PySave) -> PyQueryBuilder {
        PyQueryBuilder {
            inner: QueryBuilder::new(SelectionPart::new(
                selector,
                SelectionKind::First(save.save),
            )),
        }
    }
}
