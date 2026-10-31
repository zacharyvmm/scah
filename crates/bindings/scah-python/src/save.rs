use pyo3::exceptions::PyDeprecationWarning;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use scah_core::Save;

/// What to keep for each match of a query.
///
/// Every option except `name_only()` and `attributes=False` also keeps the
/// element's id, class, and attributes.
#[gen_stub_pyclass]
#[pyclass(module = "scah", name = "Save")]
#[derive(Clone, Copy, Debug)]
pub struct PySave {
    pub save: Save,
}

#[gen_stub_pymethods]
#[pymethods]
impl PySave {
    #[staticmethod]
    /// Keep the markup inside the element.
    pub fn only_inner_html() -> Self {
        Self {
            save: Save::only_inner_html(),
        }
    }

    #[staticmethod]
    /// Keep the source text of the element's descendants, with markup removed.
    pub fn only_raw_text() -> Self {
        Self {
            save: Save::only_raw_text(),
        }
    }

    #[staticmethod]
    /// Keep normalized, readable text: references decoded, hidden content
    /// skipped, and line breaks between blocks.
    pub fn only_text() -> Self {
        Self {
            save: Save::only_text(),
        }
    }

    #[staticmethod]
    /// Deprecated: use `only_text()`.
    pub fn only_text_content(py: Python<'_>) -> PyResult<Self> {
        PyErr::warn(
            py,
            &py.get_type::<PyDeprecationWarning>(),
            c_str!("Save.only_text_content() is deprecated; use Save.only_text()"),
            1,
        )?;
        Ok(Self {
            save: Save::only_text(),
        })
    }

    #[staticmethod]
    /// Keep inner HTML, raw text, and normalized text.
    pub fn all() -> Self {
        Self { save: Save::all() }
    }

    #[staticmethod]
    /// Keep no content, only the element's name and attributes.
    pub fn none() -> Self {
        Self { save: Save::none() }
    }

    #[staticmethod]
    /// Keep only the element's name.
    pub fn name_only() -> Self {
        Self {
            save: Save::name_only(),
        }
    }

    /// Choose each field to keep. `text_content` is a deprecated alias for `text`.
    #[new]
    #[pyo3(signature = (inner_html=false, text=None, attributes=true, *, raw_text=false, text_content=None))]
    pub fn new(
        py: Python<'_>,
        inner_html: bool,
        text: Option<bool>,
        attributes: bool,
        raw_text: bool,
        text_content: Option<bool>,
    ) -> PyResult<Self> {
        if text_content.is_some() {
            PyErr::warn(
                py,
                &py.get_type::<PyDeprecationWarning>(),
                c_str!("Save(text_content=...) is deprecated; use Save(text=...)"),
                1,
            )?;
        }

        Ok(Self {
            save: Save {
                inner_html,
                raw_text,
                text: text.or(text_content).unwrap_or(false),
                attributes,
            },
        })
    }
}
