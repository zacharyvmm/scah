use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use crate::selection::Selection;

pub trait Element {
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
                let dict = any.cast::<PyDict>().expect("The dict was just created");
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
            Ok(Some(_)) => true,
            _ => false,
        }
    }
}
