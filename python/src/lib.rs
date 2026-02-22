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
use view::{SharedString, get_memoryview_from_u8, is_subslice};

use std::ops::Range;
use std::sync::Arc;

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
    fn substring_range(parent: &[u8], substring: &str) -> Range<usize> {
        let parent_ptr = parent.as_ptr();
        let sub_ptr = substring.as_ptr();

        let start = unsafe { sub_ptr.offset_from(parent_ptr) as usize };
        let end = start + substring.len();

        start..end
    }

    #[inline]
    fn optional_substring_range(parent: &[u8], substring: Option<&str>) -> Option<Range<usize>> {
        if let Some(substring) = substring {
            Some(substring_range(parent, substring))
        } else {
            None
        }
    }

    let data = store.text_content.data();
    let text_content_rc = SharedString {
        inner: data.into_boxed_slice(),
    };
    let text_content_view = text_content_rc.as_view(py)?;
    let text_content_view_ref = text_content_view.unbind();

    let mut list = Vec::with_capacity(store.elements.len());

    let base = get_memoryview_from_u8(py, html_bytes)?;
    let base_ref = base.as_unbound();

    // Because I did not copy the tapes, I need to find the queries in the full tape

    let mut full_tape_position = 0;
    let pointer_range = py_queries
        .iter()
        .map(|q| {
            let start = full_tape_position.clone();
            full_tape_position += q.tape.len();
            (start, (*q.tape).as_ptr_range())
        })
        .collect::<Vec<_>>();

    let full_tape = Arc::new(
        py_queries
            .iter()
            .map(|q| (*q.tape).clone())
            .collect::<Vec<_>>()
            .concat(),
    );

    let find_query = |query: &str| -> Range<usize> {
        let query_ptr = query.as_ptr();
        let query_len = query.as_bytes().len();
        pointer_range
            .clone()
            .into_iter()
            .find(|(_, ptr_range)| {
                let end = unsafe { ptr_range.start.add(query_len) };
                is_subslice(ptr_range, &(query_ptr..end))
            })
            .map(|(start, ptr_range)| {
                let start = start + (unsafe { query_ptr.offset_from(ptr_range.start) }) as usize;
                start..(start + query_len)
            })
            .unwrap()
    };

    for element in store.elements {
        let attributes_iterator = element.attributes.iter().map(|a| PyAttribute {
            base: base_ref.clone_ref(py),
            key: substring_range(html_bytes, a.key),
            value: optional_substring_range(html_bytes, a.value),
        });
        let attributes = PyList::new(py, attributes_iterator)?;

        list.push(PyElement {
            base: base_ref.clone_ref(py),
            text_content_tape: text_content_view_ref.clone_ref(py),

            name: substring_range(html_bytes, element.name),
            id: optional_substring_range(html_bytes, element.id),
            class: optional_substring_range(html_bytes, element.class),
            inner_html: optional_substring_range(html_bytes, element.inner_html),
            text_content: element.text_content,

            attributes: attributes.unbind(),

            children: element
                .children
                .into_iter()
                .map(|c| {
                    (
                        // BUG: the query is not from the html, thus is not a valid range
                        // TODO: I need to copy the query tape
                        find_query(&c.query),
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
        text_content: text_content_rc,
        query_tape: full_tape,
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
