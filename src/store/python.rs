#![cfg(feature = "python")]

use super::header::{QueryError, Store};
use crate::xhtml::text_content;
use crate::{Attribute, Save, SelectionKind, SelectionPart, XHtmlElement, mut_prt_unchecked};
use pyo3::ffi::PyObject;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyString};

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

    // Just for testing
    // fn get_name(&self) -> String;
    // fn get_id(&self) -> String;
    // fn get_class(&self) -> String;
    // fn get_attributes(&self) -> Self::E;
    // fn get_attribute(&self, key: &'_ str) -> Option<String>;
    // fn get_inner_html(&self) -> String;
    // fn get_text_content(&self) -> String;
    // fn get_children(&self) -> Self::Map;
    // fn get_child(&self, key: &'_ str) -> Selection<Self::Map, Self::List>;

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

    // fn get_name(&self) -> String {
    //     let item = self.get_item(Self::NAME).unwrap().unwrap();
    //     let string= item.cast::<PyString>().unwrap();
    //     string.to_string()
    // }
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

    // TODO: Check if I really do need to fetch the values I put in it
    // Can I just return the Dict I already created?
    fn set_attribute(&mut self, key: &'_ str, value: Option<&'_ str>) -> () {
        let attributes = {
            let dict = self.get_item(Self::ATTRIBUTES);
            match dict {
                Err(_) => {
                    // init attributes
                    self.set_item(Self::ATTRIBUTES, PyDict::new(self.py()))
                        .expect("Error: was not able to create Attributes dict");
                    let any = self
                        .get_item(Self::ATTRIBUTES)
                        .expect("Just set Attributes should always return an object")
                        .expect("Item should not be None");
                    any
                }
                Ok(Some(any)) => any,
                Ok(None) => unreachable!(),
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
                Err(_) => {
                    // init children
                    self.set_item(Self::CHILDREN, PyDict::new(self.py()))
                        .expect("Error: was not able to create Children dict");
                    let any = self
                        .get_item(Self::CHILDREN)
                        .expect("Just set Children should always return an object")
                        .expect("Item should not be None");
                    any
                }
                Ok(Some(any)) => any,
                Ok(None) => unreachable!(),
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
                let list = children
                    .cast::<PyList>()
                    .expect("The list was just created");
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
                let dict = children
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
                Err(_) => {
                    return false;
                }
                Ok(Some(any)) => any,
                Ok(None) => unreachable!(),
            }
        };

        children.is_exact_instance_of::<PyDict>();
        children.is_exact_instance_of::<PyList>();

        let dict = children
            .cast::<PyDict>()
            .expect("Children is always a dict");

        match dict.get_item(key) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

#[derive(Debug)]
struct PythonElement<'py> {
    value: Bound<'py, PyDict>,
}
impl<'py, 'query> PythonElement<'py> {
    const NAME: &'static str = "name";
    const ID: &'static str = "id";
    const CLASS: &'static str = "class";
    const ATTRIBUTES: &'static str = "attributes";
    const INNER_HTML: &'static str = "innerHtml";
    const TEXT_CONTENT: &'static str = "textContent";
    const CHILDREN: &'static str = "children";

    pub fn new(context: &Python<'py>, selection: &SelectionPart<'query>) -> Self {
        let dict = PyDict::new(*context);

        let attributes = PyDict::new(*context);
        dict.set_item(Self::ATTRIBUTES, attributes);

        match selection.kind {
            SelectionKind::All(_) => {
                let children_list = PyList::empty(*context);
                dict.set_item(Self::CHILDREN, children_list);
            }
            SelectionKind::First(_) => {
                let children = PyDict::new(*context);
                dict.set_item(Self::CHILDREN, children);
            }
        }

        Self { value: dict }
    }

    #[inline]
    fn set_item<V>(&mut self, key: &'_ str, value: V) -> ()
    where
        V: pyo3::IntoPyObject<'py>,
    {
        self.value.set_item(key, value);
    }

    pub fn set_name(&mut self, value: &'_ str) -> () {
        self.set_item(Self::NAME, value);
    }

    pub fn set_id(&mut self, value: &'_ str) -> () {
        self.set_item(Self::ID, value);
    }

    pub fn set_class(&mut self, value: &'_ str) -> () {
        self.set_item(Self::CLASS, value);
    }

    pub fn set_attribute(&mut self, key: &'_ str, value: Option<&'_ str>) -> () {
        let attributes = self
            .value
            .get_item(Self::ATTRIBUTES)
            .unwrap()
            .expect("The Attributes dict should already be initialized");

        if let Some(v) = value {
            attributes.set_item(key, v);
            return;
        }
        attributes.set_item(key, true);
    }

    pub fn set_child(&mut self, key: &'_ str, value: Bound<'py, PyDict>) -> () {
        let children = self
            .value
            .get_item(Self::CHILDREN)
            .unwrap()
            .expect("The Children dict should already be initialized");
        assert!(
            children.is_exact_instance_of::<PyDict>() || children.is_exact_instance_of::<PyList>()
        );
        if children.is_exact_instance_of::<PyDict>() {}
        children.set_item(key, value);
    }
    pub fn children_contains_key(&self, key: &'_ str) -> bool {
        let children = self
            .value
            .get_item(Self::CHILDREN)
            .unwrap()
            .expect("The Children dict should already be initialized");
        children.get_item(key).is_ok()
    }

    pub fn set_inner_html(&mut self, value: &'_ str) -> () {
        self.set_item(Self::INNER_HTML, value);
    }

    pub fn set_text_content(&mut self, value: &'_ str) -> () {
        self.set_item(Self::TEXT_CONTENT, value);
    }

    pub fn from<'html>(&mut self, element: crate::XHtmlElement<'html>) {
        self.set_name(element.name);
        if element.class.is_some() {
            self.set_class(element.class.unwrap());
        }
        if element.id.is_some() {
            self.set_id(element.id.unwrap());
        }

        for attribute in element.attributes {
            self.set_attribute(attribute.name, attribute.value);
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

    // Todo: Instead of passing the context just pass in the root
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
        element: crate::XHtmlElement<'html>,
    ) -> Result<*mut Self::E, QueryError<'key>> {
        if from.is_null() {
            from = self.root.as_ptr() as *mut Self::E;
        }
        let mut new_element = PyDict::new(self.root.py());
        Self::copy_from_element(&mut new_element, element);

        // attache new element to from element
        // from.children.insert(k, v)
        let from_any = unsafe { Bound::from_borrowed_ptr(self.root.py(), from as *mut PyObject) };
        let from_dict = from_any
            .cast::<PyDict>()
            .expect("This is the only type it could be");
        let from_element = unsafe { mut_prt_unchecked!(from_dict).as_mut().unwrap() };
        //println!("Element: {from_element:?}");
        if from_element.children_contains_key(selection.source) {
            assert!(
                matches!(selection.kind, SelectionKind::All(..)),
                "It is not possible to add a single item to the store when it already exists."
            );

            let list =
                PyList::new(self.root.py(), vec![new_element]).expect("Unable to create list");
            from_element.set_child(selection.source, Selection::All(list));

            let list_of_child = from_element
                .get_item(selection.source)
                .expect("The list just got added")
                .expect("The list was not empty");
            let first = list_of_child.get_item(0);
            let pointer = match first {
                Ok(first_value_any) => {
                    let first_value = first_value_any
                        .cast::<PyDict>()
                        .expect("All list should be list of dicts.");
                    first_value.as_ptr() as *mut PyDict
                }
                Err(_) => unreachable!("Empty List"),
            };

            // BUG: This line might not work
            // I might have to refetch it
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
        // text_content_should be an Option<Range>
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
