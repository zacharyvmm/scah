use ::scah::{ChildIndex, FsmManager, Query, QueryBuilder, Reader, Store, XHtmlParser};
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyMemoryView};

mod element;
mod query;
mod save;
mod view;

use crate::element::PyMemoryViewExt;
use crate::query::{PyQuery, PyQueryBuilder, PyQueryFactory, PyQueryStatic};
use crate::save::PySave;
use element::{PyAttribute, PyElement, PyStore};
use view::{get_memoryview_from_u8, string_buffer_to_memory_view};

use std::ops::Range;

#[pyfunction]
fn parse<'py>(
    py: Python<'py>,
    html: Bound<'py, PyMemoryView>,
    queries: Bound<'py, PyAny>,
) -> PyResult<PyStore> {
    let html_bytes = {
        let buffer = PyBuffer::<u8>::get(&html)?;
        if !buffer.is_c_contiguous() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "MemoryView is not contiguous",
            ));
        }
        let slice = unsafe {
            std::slice::from_raw_parts(buffer.buf_ptr() as *const u8, buffer.len_bytes())
        };

        slice
    };

    let mut py_queries: Vec<PyQuery> = Vec::new();

    if let Ok(single_query) = queries.extract::<PyQuery>() {
        py_queries.push(single_query);
    } else if let Ok(list) = Bound::cast::<PyList>(&queries) {
        for item in list {
            py_queries.push(item.extract::<PyQuery>()?);
        }
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Query value must be a Query object or a list of Query objects",
        ));
    }

    let queries = py_queries
        .iter()
        .map(|q| q.query.clone())
        .collect::<Box<[Query]>>();

    let selectors = FsmManager::new(&queries);
    let mut parser = XHtmlParser::with_capacity(selectors, html_bytes.len());

    let mut reader = Reader::from_bytes(html_bytes);
    while parser.next(&mut reader) {}

    let store = parser.matches();

    #[inline]
    fn substring_range<'py>(
        base: &Bound<'py, PyMemoryView>,
        parent: &[u8],
        substring: &str,
    ) -> Py<PyMemoryView> {
        let parent_ptr = parent.as_ptr();
        let sub_ptr = substring.as_ptr();

        let start = unsafe { sub_ptr.offset_from(parent_ptr) as usize };
        let end = start + substring.len();

        base.slice_range(start..end)
            .expect("Their shouldn't be a conversion error")
            .unbind()
    }

    #[inline]
    fn optional_substring_range<'py>(
        base: &Bound<'py, PyMemoryView>,
        parent: &[u8],
        substring: Option<&str>,
    ) -> Option<Py<PyMemoryView>> {
        substring.and_then(|substring| Some(substring_range(base, parent, substring)))
    }

    let data = store.text_content.data();
    let text_content_view = get_memoryview_from_u8(py, &data)?;

    let mut list = Vec::with_capacity(store.elements.len());

    let base = get_memoryview_from_u8(py, html_bytes)?;
    let base_ref = base.as_unbound();

    for element in store.elements {
        let attributes_iterator = element.attributes.iter().map(|a| PyAttribute {
            key: substring_range(&base, html_bytes, a.key),
            value: optional_substring_range(&base, html_bytes, a.value),
        });
        let attributes = PyList::new(py, attributes_iterator)?;

        list.push(PyElement {
            name: substring_range(&base, html_bytes, element.name),
            id: optional_substring_range(&base, html_bytes, element.id),
            class: optional_substring_range(&base, html_bytes, element.class),
            inner_html: optional_substring_range(&base, html_bytes, element.inner_html),
            text_content: element.text_content.and_then(|tc| {
                Some(
                    text_content_view
                        .slice_range(tc)
                        .expect("Their shouldn't be a conversion error")
                        .unbind(),
                )
            }),

            attributes: attributes.unbind(),

            children: element
                .children
                .into_iter()
                .map(|c| {
                    (
                        substring_range(&base, html_bytes, c.query),
                        match c.index {
                            ChildIndex::One(index) => vec![index],
                            ChildIndex::Many(vec) => vec,
                        },
                    )
                })
                .collect(),
        });
    }

    Ok(PyStore {
        elements: PyList::new(py, list)?.unbind(),
        text_content: text_content_view.unbind(),
    })
}

#[pymodule]
fn scah(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_class::<PySave>()?;
    m.add_class::<PyQuery>()?;
    m.add_class::<PyQueryBuilder>()?;
    m.add_class::<PyQueryStatic>()?;
    m.add_class::<PyQueryFactory>()?;
    m.add_class::<PyElement>()?;
    m.add_class::<PyAttribute>()?;
    m.add_class::<PyStore>()?;
    Ok(())
}
