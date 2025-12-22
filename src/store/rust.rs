use crate::css::{SelectionKind, SelectionPart};

use super::header::{QueryError, Store};
use crate::mut_prt_unchecked;
use std::collections::HashMap;
use std::ops::Index;
use std::ptr;

#[derive(Debug, PartialEq)]
pub enum ValueKind {
    SingleItem,
    List,
}

#[derive(Debug, PartialEq)]
pub struct SelectionValue<'html, 'query> {
    pub(crate) kind: ValueKind,
    pub(crate) list: Vec<Element<'html, 'query>>,
}

impl<'html, 'query: 'html, 'key> SelectionValue<'html, 'query> {
    fn one(&self) -> Result<&Element<'html, 'query>, QueryError<'key>> {
        match self.kind {
            ValueKind::SingleItem => {
                return Ok(&self.list[0]);
            }
            ValueKind::List => Err(QueryError::NotASingleElement),
        }
    }

    fn list(&'html self) -> Result<&'html [Element<'html, 'query>], QueryError<'key>> {
        match self.kind {
            ValueKind::SingleItem => Err(QueryError::NotAList),
            ValueKind::List => Ok(self.list.as_slice()),
        }
    }

    #[inline]
    pub fn value(&self) -> Result<&Element<'html, 'query>, QueryError<'key>> {
        self.one()
    }

    /// List operations
    pub fn iter(
        &'html self,
    ) -> Result<impl Iterator<Item = &'html Element<'html, 'query>>, QueryError<'key>> {
        self.list().map(|vec| vec.iter())
    }

    pub fn push(
        &'html mut self,
        element: Element<'html, 'query>,
    ) -> Result<*mut Element<'html, 'query>, QueryError<'key>> {
        match self.kind {
            ValueKind::SingleItem => Err(QueryError::NotAList),
            ValueKind::List => {
                let vec = &mut self.list;
                vec.push(element);
                let index = vec.len() - 1;
                let last_element = &mut vec[index];
                let pointer = ptr::from_mut(last_element);

                Ok(pointer)
            }
        }
    }

    pub fn len(&'html self) -> Result<usize, QueryError<'key>> {
        self.list().map(|vec| vec.len())
    }

    pub fn get(
        &'html self,
        index: usize,
    ) -> Result<&'html Element<'html, 'query>, QueryError<'key>> {
        self.list()?
            .get(index)
            .ok_or_else(|| QueryError::IndexOutOfBounds {
                index,
                len: self.list().unwrap().len(),
            })
    }
}

#[derive(Debug, PartialEq)]
pub struct Element<'html, 'query> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub attributes: Vec<(&'html str, Option<&'html str>)>,
    pub inner_html: Option<&'html str>,
    pub text_content: Option<String>,
    // Store Selection directly to enable Index trait
    pub(crate) children: HashMap<&'query str, SelectionValue<'html, 'query>>,
}

impl<'html, 'query, 'key> Element<'html, 'query> {
    /// Safe primary access method
    pub fn get(
        &'html self,
        key: &'key str,
    ) -> Result<&'html SelectionValue<'html, 'query>, QueryError<'key>> {
        self.children.get(key).ok_or(QueryError::KeyNotFound(key))
    }

    /// Panicking accessor for known keys
    pub fn select(&'html self, key: &'key str) -> &'html SelectionValue<'html, 'query> {
        self.get(key).unwrap()
    }

    /// Check existence without error
    pub fn contains_key(&self, key: &'key str) -> bool {
        self.children.contains_key(key)
    }
}

impl<'html, 'query> Index<&'query str> for Element<'html, 'query> {
    type Output = SelectionValue<'html, 'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        &self.children[key] // Panics if key not found
    }
}

impl<'html, 'query> Index<usize> for SelectionValue<'html, 'query> {
    type Output = Element<'html, 'query>;

    fn index(&self, index: usize) -> &Self::Output {
        match self.kind {
            ValueKind::SingleItem => panic!("Cannot use usize index on single element"),
            ValueKind::List => &self.list[index], // Panics if out of bounds
        }
    }
}

impl<'html, 'query> Index<&'query str> for SelectionValue<'html, 'query> {
    type Output = SelectionValue<'html, 'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        match self.kind {
            ValueKind::SingleItem => &self.list[0][key], // Panics if key not found
            ValueKind::List => panic!("Cannot chain string index on list selection"),
        }
    }
}

pub struct ElementBuilder<'html, 'query> {
    name: &'html str,
    class: Option<&'html str>,
    id: Option<&'html str>,
    attributes: Vec<(&'html str, Option<&'html str>)>,
    inner_html: Option<&'html str>,
    text_content: Option<String>,
    children: HashMap<&'query str, SelectionValue<'html, 'query>>,
}

impl<'html, 'query> ElementBuilder<'html, 'query> {
    pub fn new(name: &'html str) -> Self {
        Self {
            name,
            class: None,
            id: None,
            attributes: Vec::new(),
            inner_html: None,
            text_content: None,
            children: HashMap::new(),
        }
    }

    pub fn class(mut self, class: &'html str) -> Self {
        self.class = Some(class);
        self
    }

    pub fn id(mut self, id: &'html str) -> Self {
        self.id = Some(id);
        self
    }

    pub fn attribute(mut self, name: &'html str, value: Option<&'html str>) -> Self {
        self.attributes.push((name, value));
        self
    }

    pub fn inner_html(mut self, html: &'html str) -> Self {
        self.inner_html = Some(html);
        self
    }

    pub fn text_content(mut self, text: String) -> Self {
        self.text_content = Some(text);
        self
    }

    /// Add a single child
    pub fn child(mut self, key: &'query str, element: Element<'html, 'query>) -> Self {
        self.children.insert(
            key,
            SelectionValue {
                kind: ValueKind::SingleItem,
                list: vec![element],
            },
        );
        self
    }

    /// Add multiple children
    pub fn children(mut self, key: &'query str, elements: Vec<Element<'html, 'query>>) -> Self {
        self.children.insert(
            key,
            SelectionValue {
                kind: ValueKind::List,
                list: elements,
            },
        );
        self
    }

    pub fn build(self) -> Element<'html, 'query> {
        Element {
            name: self.name,
            class: self.class,
            id: self.id,
            attributes: self.attributes,
            inner_html: self.inner_html,
            text_content: self.text_content,
            children: self.children,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RustStore<'html, 'query> {
    pub(crate) root: Box<Element<'html, 'query>>,
}

impl<'html, 'query: 'html> Store<'html, 'query> for RustStore<'html, 'query> {
    type E = Element<'html, 'query>;

    fn new() -> Self {
        Self {
            root: Box::new(Element {
                name: "root",
                class: None,
                id: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: HashMap::new(),
            }),
        }
    }

    fn root(&mut self) -> *mut Element<'html, 'query> {
        mut_prt_unchecked!(self.root.as_ref())
    }

    fn push<'key>(
        &mut self,
        selection: &SelectionPart<'query>,
        mut from: *mut Element<'html, 'query>,
        element: crate::XHtmlElement<'html>,
    ) -> Result<*mut Element<'html, 'query>, QueryError<'key>> {
        if from.is_null() {
            from = mut_prt_unchecked!(&self.root);
        }
        let new_element: Element<'html, 'query> = Element {
            name: element.name,
            class: element.class,
            id: element.id,
            attributes: element
                .attributes
                .iter()
                .map(|a| (a.name, a.value))
                .collect(),
            inner_html: None,
            text_content: None,
            children: HashMap::new(),
        };

        // attache new element to from element
        // from.children.insert(k, v)
        let from_element = unsafe { from.as_mut() }.unwrap();
        //println!("Element: {from_element:?}");
        let children_map = &mut from_element.children;

        if children_map.contains_key(selection.source) {
            match selection.kind {
                SelectionKind::First(_) => panic!(
                    "It is not possible to add a single item to the store when it already exists."
                ),
                SelectionKind::All(_) => {
                    assert!(matches!(
                        children_map[selection.source].kind,
                        ValueKind::List
                    ));
                    //assert!(from.children[selection.source].iter()?.collect().len() > 0);
                    let const_list = &children_map[selection.source];
                    let list = unsafe { mut_prt_unchecked!(const_list).as_mut() }.unwrap();

                    return list.push(new_element);
                }
            }
        }
        children_map.insert(
            selection.source,
            match selection.kind {
                SelectionKind::First(_) => SelectionValue {
                    kind: ValueKind::SingleItem,
                    list: vec![new_element],
                },
                SelectionKind::All(_) => SelectionValue {
                    kind: ValueKind::List,
                    list: vec![new_element],
                },
            },
        );

        let selection_value = &children_map[selection.source];

        return match selection_value.kind {
            ValueKind::SingleItem => {
                let mut_element: *mut Element<'html, 'query> =
                    mut_prt_unchecked!(&selection_value.list[0]);
                return Ok(mut_element);
            }
            ValueKind::List => {
                let index = selection_value.list.len() - 1;
                let last_element = &selection_value.list[index];

                let mut_element = mut_prt_unchecked!(last_element);
                Ok(mut_element)
            }
        };
    }

    fn set_content<'key>(
        &mut self,
        element: *mut Self::E,
        inner_html: Option<&'html str>,
        text_content: Option<String>,
    ) -> () {
        let ele = unsafe { element.as_mut() }.unwrap();
        ele.inner_html = inner_html;
        ele.text_content = text_content;
    }
}

#[cfg(test)]
mod tests {

    use crate::{XHtmlElement, css::Save, utils::Reader};

    use super::*;

    #[test]
    fn test_element_access() -> Result<(), QueryError<'static>> {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello".to_string()).build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First".to_string()).build(),
                    ElementBuilder::new("li").text_content("Second".to_string()).build(),
                ],
            )
            .build();

        // Different ways of acessing fields
        assert_eq!(doc.get("title")?.value()?.name, "h1");
        assert_eq!(doc["title"].value()?.name, "h1");

        assert_eq!(doc.get("items")?.len()?, 2);
        assert_eq!(doc["items"].len()?, 2);

        assert_eq!(doc.get("items")?.get(0)?.text_content, Some("First".to_string()));
        assert_eq!(doc["items"][0].text_content, Some("First".to_string()));

        // Iterators for All Selections
        let items_iter1 = doc.get("items")?.iter()?;
        assert_eq!(
            items_iter1.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First".to_string()).build(),
                &ElementBuilder::new("li").text_content("Second".to_string()).build(),
            ]
        );

        let items_iter2 = doc["items"].iter()?;
        assert_eq!(
            items_iter2.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First".to_string()).build(),
                &ElementBuilder::new("li").text_content("Second".to_string()).build(),
            ]
        );

        assert!(!doc.contains_key("optional"));

        assert_eq!(
            doc.get("optional"),
            Err(QueryError::KeyNotFound("optional"))
        );

        Ok(())
    }

    #[test]
    #[should_panic(expected = "no entry found for key")]
    fn test_non_existing_key_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello".to_string()).build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First".to_string()).build(),
                    ElementBuilder::new("li").text_content("Second".to_string()).build(),
                ],
            )
            .build();

        assert!(!doc.contains_key("non-existing"));
        assert_eq!(
            doc.get("non-existing"),
            Err(QueryError::KeyNotFound("non-existing"))
        );

        let non_existing = &doc["non-existing"];
    }

    #[test]
    #[should_panic(expected = "Cannot use usize index on single element")]
    fn test_index_on_single_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello".to_string()).build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First".to_string()).build(),
                    ElementBuilder::new("li").text_content("Second".to_string()).build(),
                ],
            )
            .build();

        assert_eq!(doc.get("title").unwrap().get(0), Err(QueryError::NotAList));

        let non_existing_list = &doc["title"][0];
    }

    #[test]
    #[should_panic(expected = "Cannot chain string index on list selection")]
    fn test_key_on_vec_element_access() {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello".to_string()).build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First".to_string()).build(),
                    ElementBuilder::new("li").text_content("Second".to_string()).build(),
                ],
            )
            .build();

        // No Safe equivalent because of the types

        let non_existing_key = &doc["items"]["li"];
    }

    #[test]
    fn test_build_tree() {
        /*
         * root -> a.class --> span#id --> p
         *      |          \-> p
         *      \-> div
         */
        let mut store = RustStore::new();

        // SETUP Elements
        let first = XHtmlElement::from(&mut Reader::new("a class=\"class\""));
        let second = XHtmlElement::from(&mut Reader::new("span id=\"id\""));
        let second_extended = XHtmlElement::from(&mut Reader::new("p"));
        let second_alternate = XHtmlElement::from(&mut Reader::new("p"));
        let first_alternate = XHtmlElement::from(&mut Reader::new("div"));

        let selection_first = SelectionPart::new(
            "a",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );

        let selection_first_alternate = SelectionPart::new(
            "div",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );

        let selection_second_alternate = SelectionPart::new(
            "p",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );

        let selection_second = SelectionPart::new(
            "span",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );

        let selection_second_extended = SelectionPart::new(
            "p",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );

        let mut last_element = store
            .push(&selection_first, std::ptr::null_mut(), first)
            .expect("Should be able to add an element");
        let _ = store
            .push(
                &selection_first_alternate,
                std::ptr::null_mut(),
                first_alternate,
            )
            .expect("Should be able to add an element");
        let _ = store
            .push(&selection_second_alternate, last_element, second_alternate)
            .expect("Should be able to add an element");
        last_element = store
            .push(&selection_second, last_element, second)
            .expect("Should be able to add an element");
        let _ = store
            .push(&selection_second_extended, last_element, second_extended)
            .expect("Should be able to add an element");

        println!("{store:?}");

        /*
         * root -> a.class --> span#id --> p
         *      |          \-> p
         *      \-> div
         */
        assert_eq!(
            *store.root,
            Element {
                name: "root",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: HashMap::from([
                    (
                        "a",
                        SelectionValue {
                            kind: ValueKind::List,
                            list: vec![Element {
                                name: "a",
                                id: None,
                                class: Some("class"),
                                attributes: vec![],
                                inner_html: None,
                                text_content: None,
                                children: HashMap::from([
                                    (
                                        "span",
                                        SelectionValue {
                                            kind: ValueKind::List,
                                            list: vec![Element {
                                                name: "span",
                                                class: None,
                                                id: Some("id"),
                                                attributes: vec![],
                                                inner_html: None,
                                                text_content: None,
                                                children: HashMap::from([(
                                                    "p",
                                                    SelectionValue {
                                                        kind: ValueKind::List,
                                                        list: vec![Element {
                                                            name: "p",
                                                            class: None,
                                                            id: None,
                                                            attributes: vec![],
                                                            inner_html: None,
                                                            text_content: None,
                                                            children: HashMap::new()
                                                        }]
                                                    }
                                                )]),
                                            }]
                                        }
                                    ),
                                    (
                                        "p",
                                        SelectionValue {
                                            kind: ValueKind::List,
                                            list: vec![Element {
                                                name: "p",
                                                class: None,
                                                id: None,
                                                attributes: vec![],
                                                inner_html: None,
                                                text_content: None,
                                                children: HashMap::new(),
                                            }]
                                        }
                                    )
                                ])
                            }]
                        }
                    ),
                    (
                        "div",
                        SelectionValue {
                            kind: ValueKind::List,
                            list: vec![Element {
                                name: "div",
                                id: None,
                                class: None,
                                attributes: vec![],
                                inner_html: None,
                                text_content: None,
                                children: HashMap::new()
                            }]
                        }
                    )
                ])
            }
        );
    }
}
