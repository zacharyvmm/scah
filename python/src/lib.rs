use ::onego::{FsmManager, QueryBuilder, Reader, SelectionPart, Store, XHtmlParser, ChildIndex};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyMemoryView};
use pyo3::buffer::PyBuffer;

mod element;
mod query;
mod save;
mod selection;
mod view;

use crate::query::{PyQuery, PyQueryBuilder, PyQueryFactory, PyQueryStatic};
use crate::save::PySave;
use view::{string_buffer_to_memory_view, get_memoryview_from_str};
//use crate::store::PythonStore;

use std::ops::Range;

type StrRange = Range<usize>;
type OptionalStrRange = Option<StrRange>;

pub struct Attribute {
    key: StrRange,
    value: OptionalStrRange,
}

#[pyclass(name="Element")]
pub struct PyElement {
    base: Py<PyMemoryView>,
    text_content_tape: Py<PyMemoryView>,

    name: StrRange,
    class: OptionalStrRange,
    id: OptionalStrRange,
    attributes: Vec<Attribute>,
    children: Vec<(StrRange, Vec<usize>)>,

    inner_html: OptionalStrRange,
    text_content: OptionalStrRange,
}

#[pyfunction]
fn parse<'py>(
    py: Python<'py>,
    html: &str,
    //html: Bound<'py, PyMemoryView>,
    queries: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    // let html_bytes = {
    //     let buffer = PyBuffer::<u8>::get(&html)?;
    //     if !buffer.is_c_contiguous() {
    //         return Err(pyo3::exceptions::PyValueError::new_err("MemoryView is not contiguous"));
    //     }
    //     let slice = unsafe {
    //         std::slice::from_raw_parts(buffer.buf_ptr() as *const u8, buffer.len_bytes())
    //     };

    //     slice
    // };

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

    let mut built_queries = Vec::with_capacity(py_queries.len());

    // We need to keep the Queries alive, which means the Strings inside them must be alive.
    // The `py_queries` vector owns the `PyQuery` objects (clones).

    // We iterate through py_queries and build Rust Query objects borrowing from them.
    for py_query in &py_queries {
        if py_query.builder.is_empty() {
            continue;
        }

        // Convert Vec<SelectionPart<String>> to Vec<SelectionPart<&str>>
        let parts_ref: Vec<SelectionPart<&str>> = py_query
            .builder
            .iter()
            .map(|part| SelectionPart {
                source: part.source.as_str(),
                kind: part.kind,
                parent: part.parent,
                children: part.children.clone(),
            })
            .collect();

        // Reconstruct QueryBuilder for &str
        let builder = QueryBuilder::from_list(parts_ref);

        built_queries.push(builder.build());
    }

    let selectors = FsmManager::new(&built_queries);
    let mut parser = XHtmlParser::with_capacity(selectors, html.len());

    let mut reader = Reader::new(&html);
    while parser.next(&mut reader) {}

    let store = parser.matches();
    let text_content_view = string_buffer_to_memory_view(py, store.text_content.data())?;
    let text_content_view_ref: &Py<PyMemoryView> = text_content_view.as_unbound();

    let mut list = Vec::with_capacity(store.elements.len());

    #[inline]
    fn substring_range(parent: &str, substring: &str) -> Range<usize> {
        let parent_ptr = parent.as_ptr();
        let sub_ptr = substring.as_ptr();

        let start = unsafe { sub_ptr.offset_from(parent_ptr) as usize };
        let end = start + substring.len();

        start..end
    }

    #[inline]
    fn optional_substring_range(parent: &str, substring: Option<&str>) -> Option<Range<usize>> {
        if let Some(substring) = substring {
            Some(substring_range(parent, substring))
        } else {
            None
        }
    }

    let base_ref: &Py<PyMemoryView> = &unsafe { get_memoryview_from_str(py, html) }?.unbind();

    for element in store.elements {
        list.push(PyElement {
            base: base_ref.clone_ref(py),
            text_content_tape: text_content_view_ref.clone_ref(py),

            name: substring_range(&html, element.name),
            id: optional_substring_range(&html, element.id),
            class: optional_substring_range(&html, element.class),
            inner_html: optional_substring_range(&html, element.inner_html),
            text_content: element.text_content,

            attributes: element.attributes.iter().map(|a| Attribute {
                key: substring_range(html, a.key),
                value: optional_substring_range(html, a.value),
            }).collect(),

            children: element.children.into_iter().map(|c| (substring_range(&html, c.query), match c.index {
                ChildIndex::One(index) => vec![index],
                ChildIndex::Many(vec) => vec,
            })).collect()
        });
    }

    Ok(PyList::new(py, list)?)
}

#[pymodule]
fn onego(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_class::<PySave>()?;
    m.add_class::<PyQuery>()?;
    m.add_class::<PyQueryBuilder>()?;
    m.add_class::<PyQueryStatic>()?;
    m.add_class::<PyQueryFactory>()?;
    m.add_class::<PyElement>()?;
    Ok(())
}
