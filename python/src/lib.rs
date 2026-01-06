use ::onego::{
    Attribute, FsmManager, QueryBuilder, QueryError, Reader, Save, SelectionKind, SelectionPart,
    Store, XHtmlElement, XHtmlParser,
};
use pyo3::ffi::PyObject;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

macro_rules! mut_prt_unchecked {
    ($e:expr) => {{
        #[inline(always)]
        fn cast<T>(r: &T) -> *mut T {
            r as *const T as *mut T
        }
        cast($e)
    }};
}

#[derive(Debug)]
pub enum Selection<M, L> {
    First(M),
    All(L),
}

trait Element {
    type MapPointer;
    type Map;
    type List;

    const NAME: &'static str = "name";
    const ID: &'static str = "id";
    const CLASS: &'static str = "class";
    const ATTRIBUTES: &'static str = "attributes";
    const INNER_HTML: &'static str = "innerHtml";
    const TEXT_CONTENT: &'static str = "textContent";
    const CHILDREN: &'static str = "children";

    fn set_name(&mut self, value: &'_ str) -> ();
    fn set_id(&mut self, value: &'_ str) -> ();
    fn set_class(&mut self, value: &'_ str) -> ();
    fn set_inner_html(&mut self, value: &'_ str) -> ();
    fn set_text_content(&mut self, value: &'_ str) -> ();

    fn children_contains_key(&self, key: &'_ str) -> bool;

    fn set_attribute(&mut self, key: &'_ str, value: Option<&'_ str>) -> ();
    fn set_child(
        &mut self,
        key: &'_ str,
        value: Selection<Self::Map, Self::List>,
    ) -> *mut Self::MapPointer;
}

impl<'py> Element for Bound<'py, PyDict> {
    type MapPointer = PyDict;
    type Map = Bound<'py, PyDict>;
    type List = Bound<'py, PyList>;

    fn set_name(&mut self, value: &'_ str) -> () {
        let _ = self.set_item(Self::NAME, value);
    }
    fn set_id(&mut self, value: &'_ str) -> () {
        let _ = self.set_item(Self::ID, value);
    }
    fn set_class(&mut self, value: &'_ str) -> () {
        let _ = self.set_item(Self::CLASS, value);
    }
    fn set_inner_html(&mut self, value: &'_ str) -> () {
        let _ = self.set_item(Self::INNER_HTML, value);
    }
    fn set_text_content(&mut self, value: &'_ str) -> () {
        let _ = self.set_item(Self::TEXT_CONTENT, value);
    }

    fn set_attribute(&mut self, key: &'_ str, value: Option<&'_ str>) -> () {
        let attributes = {
            let dict = self.get_item(Self::ATTRIBUTES);
            match dict {
                Ok(None) | Err(_) => {
                    self.set_item(Self::ATTRIBUTES, PyDict::new(self.py()))
                        .expect("Error: was not able to create Attributes dict");
                    let any = self
                        .get_item(Self::ATTRIBUTES)
                        .expect("Just set Attributes should always return an object")
                        .expect("Item should not be None");
                    any
                }
                Ok(Some(any)) => any,
            }
        };

        let dict = attributes
            .cast::<PyDict>()
            .expect("Attributes is always a dict");

        let _ = dict.set_item(key, value);
    }

    fn set_child(
        &mut self,
        key: &'_ str,
        value: Selection<Self::Map, Self::List>,
    ) -> *mut Self::MapPointer {
        let children = {
            let dict = self.get_item(Self::CHILDREN);
            match dict {
                Ok(None) | Err(_) => {
                    self.set_item(Self::CHILDREN, PyDict::new(self.py()))
                        .expect("Error: was not able to create Children dict");
                    let any = self
                        .get_item(Self::CHILDREN)
                        .expect("Just set Children should always return an object")
                        .expect("Item should not be None");
                    any
                }
                Ok(Some(any)) => any,
            }
        };

        let dict = children
            .cast::<PyDict>()
            .expect("Children is always a dict");

        match value {
            Selection::All(v) => {
                dict.set_item(key, v)
                    .expect("Was unable to set a value of list");
                let any = dict
                    .get_item(key)
                    .expect("The Child was just set, thus it should exist")
                    .unwrap();
                let list = any.cast::<PyList>().expect("The list was just created");
                let first = list.get_item(0);
                match first {
                    Ok(first_value_any) => {
                        let first_value = first_value_any
                            .cast::<PyDict>()
                            .expect("All list should be list of dicts.");
                        first_value.as_ptr() as *mut PyDict
                    }
                    Err(_) => unreachable!("Empty List"),
                }
            }
            Selection::First(v) => {
                dict.set_item(key, v)
                    .expect("Was unable to set a value of dict");
                let any = dict
                    .get_item(key)
                    .expect("The Child was just set, thus it should exist")
                    .unwrap();
                let dict = any
                    .cast::<PyDict>()
                    .expect("The dict was just created");
                dict.as_ptr() as *mut PyDict
            }
        }
    }

    fn children_contains_key(&self, key: &'_ str) -> bool {
        let children = {
            let dict = self.get_item(Self::CHILDREN);
            match dict {
                Ok(None) | Err(_) => {
                    return false;
                }
                Ok(Some(any)) => any,
            }
        };

        // children.is_exact_instance_of::<PyDict>();
        // children.is_exact_instance_of::<PyList>();

        let dict = children
            .cast::<PyDict>()
            .expect("Children is always a dict");

        match dict.get_item(key) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
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
        selection: &SelectionPart<'query>,
        mut from: *mut Self::E,
        element: XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>> {
        if from.is_null() {
            from = self.root.as_ptr() as *mut Self::E;
        }
        let mut new_element = PyDict::new(self.root.py());
        Self::copy_from_element(&mut new_element, element);

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

#[pyfunction]
fn parse<'py>(
    py: Python<'py>,
    html: &str,
    queries: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let keys_list = queries.keys();
    // Assuming a single query
    let query_any = keys_list
        .get_item(0)
        .expect("Should have at least a single key");
    let query_string = query_any
        .cast::<PyString>()
        .expect("A key must be a string");
    let query = query_string.extract().unwrap();
    let queries = &[QueryBuilder::new(SelectionPart::new(
        query,
        SelectionKind::All(Save {
            inner_html: true,
            text_content: true,
        }),
    ))
    .build()];
    let store = PythonStore::new(py);
    let selectors = FsmManager::new(store, queries);
    let mut parser = XHtmlParser::new(selectors);

    let mut reader = Reader::new(html);
    while parser.next(&mut reader) {}

    let out_store = parser.matches();
    Ok(out_store.root)
}

#[pymodule]
fn onego(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    Ok(())
}
