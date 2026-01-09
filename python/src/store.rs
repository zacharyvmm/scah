use ::onego::{
    Attribute, QueryError, SelectionKind, Store, XHtmlElement, QuerySection,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use crate::element::Element;
use crate::selection::Selection;

macro_rules! mut_prt_unchecked {
    ($e:expr) => {{
        #[inline(always)]
        fn cast<T>(r: &T) -> *mut T {
            r as *const T as *mut T
        }
        cast($e)
    }};
}

pub struct PythonStore<'py> {
    pub(crate) root: Bound<'py, PyDict>,
}

impl<'py> PythonStore<'py> {
    fn copy_from_element(dict: &mut impl Element, element: XHtmlElement) {
        dict.set_name(element.name);
        if let Some(id) = element.id {
            dict.set_id(id);
        }
        if let Some(class) = element.class {
            dict.set_class(class);
        }

        for Attribute { name, value } in element.attributes {
            dict.set_attribute(name, value);
        }
    }

    fn set_inner_html(dict: &mut impl Element, inner_html: &str) {
        dict.set_inner_html(inner_html);
    }

    fn set_text_content(dict: &mut impl Element, text_content: &str) {
        dict.set_text_content(text_content);
    }
}

impl<'py, 'html, 'query: 'html> Store<'html, 'query> for PythonStore<'py> {
    type E = PyDict;
    type Context = Python<'py>;

    fn new(context: Self::Context) -> Self {
        Self {
            root: PyDict::new(context),
        }
    }

    fn root(&mut self) -> *mut Self::E {
        self.root.as_ptr() as *mut Self::E
    }

    fn push<'key>(
        &mut self,
        selection: &QuerySection<'query>,
        mut from: *mut Self::E,
        element: XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>> {
        if from.is_null() {
            from = self.root.as_ptr() as *mut Self::E;
        }
        let mut new_element = PyDict::new(self.root.py());
        Self::copy_from_element(&mut new_element, element);

        use pyo3::ffi::PyObject;
        let from_any = unsafe { Bound::from_borrowed_ptr(self.root.py(), from as *mut PyObject) };
        let from_dict = from_any
            .cast::<PyDict>()
            .expect("This is the only type it could be");
        let from_element = unsafe { mut_prt_unchecked!(from_dict).as_mut().unwrap() };

        if from_element.children_contains_key(selection.source) {
            assert!(
                matches!(selection.kind, SelectionKind::All(..)),
                "It is not possible to add a single item to the store when it already exists."
            );

            const CHILDREN: &'static str = "children";
            let children_any = from_element
                .get_item(CHILDREN)
                .expect("children should be initialized")
                .expect("Value should not be None");
            let children = children_any
                .cast::<PyDict>()
                .expect("Children is always a dict");

            let list_of_child = children
                .get_item(selection.source)
                .expect("The list just got added")
                .expect("The list was not empty");
            let list = list_of_child
                .cast::<PyList>()
                .expect("children should be a list");

            list.append(new_element).expect("Failed to append");
            let len = list.len();

            let first = list.get_item(len - 1);
            let pointer = match first {
                Ok(first_value_any) => {
                    let first_value = first_value_any
                        .cast::<PyDict>()
                        .expect("All list should be list of dicts.");
                    first_value.as_ptr() as *mut PyDict
                }
                Err(_) => unreachable!("Empty List"),
            };

            return Ok(pointer);
        }

        let pointer = from_element.set_child(
            selection.source,
            match selection.kind {
                SelectionKind::First(_) => Selection::First(new_element),
                SelectionKind::All(_) => {
                    let list = PyList::new(self.root.py(), vec![new_element])
                        .expect("Unable to create list");
                    Selection::All(list)
                }
            },
        );

        Ok(pointer)
    }

    fn set_content<'key>(
        &mut self,
        element: *mut Self::E,
        inner_html: Option<&'html str>,
        text_content: Option<String>,
    ) -> () {
        use pyo3::ffi::PyObject;
        let ele_any = unsafe { Bound::from_borrowed_ptr(self.root.py(), element as *mut PyObject) };
        let ele_dict = ele_any
            .cast::<PyDict>()
            .expect("This is the only type it could be");
        let ele = unsafe { mut_prt_unchecked!(ele_dict).as_mut().unwrap() };

        if let Some(ih) = inner_html {
            Self::set_inner_html(ele, ih);
        }

        if let Some(tc) = text_content {
            Self::set_text_content(ele, tc.as_str());
        }
    }
}
