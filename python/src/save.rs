use ::scah::Save;
use pyo3::prelude::*;

#[pyclass(name = "Save")]
#[derive(Clone, Copy, Debug)]
pub struct PySave {
    pub save: Save,
}

#[pymethods]
impl PySave {
    #[staticmethod]
    pub fn only_inner_html() -> Self {
        Self {
            save: Save::only_inner_html(),
        }
    }

    #[staticmethod]
    pub fn only_text_content() -> Self {
        Self {
            save: Save::only_text_content(),
        }
    }

    #[staticmethod]
    pub fn all() -> Self {
        Self { save: Save::all() }
    }

    #[staticmethod]
    pub fn none() -> Self {
        Self { save: Save::none() }
    }

    #[new]
    #[pyo3(signature = (inner_html=false, text_content=false))]
    pub fn new(inner_html: bool, text_content: bool) -> Self {
        Self {
            save: Save {
                inner_html,
                text_content,
            },
        }
    }
}
