use crate::css::SelectionKind;
use crate::runner::element::{Attribute, XHtmlElement};
use crate::xhtml::text_content::{self, TextContent};

use crate::{QuerySection, dbg_print, to_str};
use std::ops::{Index, Range};

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError<'key> {
    KeyNotFound(&'key str),
    NotASingleElement,
    NotAList,
    IndexOutOfBounds { index: usize, len: usize },
}

#[derive(Debug, PartialEq)]
pub enum ChildIndex {
    One(usize),
    Many(Vec<usize>),
}

impl ChildIndex {
    pub fn value(&self) -> Result<usize, QueryError<'static>> {
        match self {
            ChildIndex::One(index) => Ok(*index),
            ChildIndex::Many(_) => Err(QueryError::NotASingleElement),
        }
    }

    pub fn iter(&self) -> Result<std::slice::Iter<'_, usize>, QueryError<'static>> {
        match self {
            ChildIndex::Many(indices) => Ok(indices.iter()),
            ChildIndex::One(_) => Err(QueryError::NotAList),
        }
    }
}

impl Index<usize> for ChildIndex {
    type Output = usize;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            ChildIndex::Many(list) => &list[index],
            ChildIndex::One(_) => panic!("Cannot use usize index on single element"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Child<'query> {
    pub query: &'query str,
    pub index: ChildIndex,
}

impl<'query> Child<'query> {
    pub fn value(&self) -> Result<usize, QueryError<'static>> {
        self.index.value()
    }

    pub fn iter(&self) -> Result<std::slice::Iter<'_, usize>, QueryError<'static>> {
        self.index.iter()
    }
}

impl<'query> Index<usize> for Child<'query> {
    type Output = usize;

    fn index(&self, index: usize) -> &Self::Output {
        &self.index[index]
    }
}

type Children<'query> = Vec<Child<'query>>;

#[derive(Debug, PartialEq)]
pub struct Element<'html, 'query> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub inner_html: Option<&'html str>,
    pub text_content: Option<Range<u32>>,
    // Store Selection directly to enable Index trait
    attributes: Range<u32>,
    pub children: Children<'query>,
}

impl<'html, 'query, 'key> Element<'html, 'query> {
    /// Safe primary access method
    pub fn get(&'html self, key: &'key str) -> Result<&'html ChildIndex, QueryError<'key>> {
        if let Some(index) = self.index_of_child_with_key(key) {
            return Ok(&self.children[index].index);
        }

        Err(QueryError::KeyNotFound(key))
    }

    /// Panicking accessor for known keys
    pub fn select(&'html self, key: &'key str) -> &'html ChildIndex {
        self.get(key).unwrap()
    }

    /// Check existence without error
    pub fn index_of_child_with_key(&self, key: &'key str) -> Option<usize> {
        for (index, child) in self.children.iter().enumerate() {
            if child.query == key {
                return Some(index);
            }
        }
        None
    }
}

#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    pub elements: Vec<Element<'html, 'query>>,
    pub(crate) attributes: Vec<Attribute<'html>>,
    pub text_content: TextContent,
}
const NULL_RANGE: Range<u32> = 0..0;

impl<'html, 'query: 'html> Store<'html, 'query> {
    pub fn new(capacity:  usize) -> Self {
        let mut elements = Vec::with_capacity(capacity/3);
        elements.push(Element {
                name: "root",
                class: None,
                id: None,
                inner_html: None,
                attributes: NULL_RANGE,
                text_content: None,
                children: vec![],
            });
        Self {
            elements,
            text_content: TextContent::new(capacity/3),
            attributes: Vec::with_capacity(capacity/3),
        }
    }

    pub fn text_content(&self, element: &Element<'html, 'query>) -> Option<&str> {
        if let Some(content) = &element.text_content {
            let start = content.start as usize;
            let end = content.end as usize;
            Some(&self.text_content.slice(start..end))
        } else {None}
    }

    pub fn attributes(&self, element: &Element<'html, 'query>) -> Option<&[Attribute<'html>]> {
        if element.attributes.start == element.attributes.end {
            return None;
        }
        let start = element.attributes.start as usize;
        let end = element.attributes.end as usize;
        Some(&self.attributes[start..end])
    }

    pub fn push(
        &mut self,
        selection: &QuerySection<'query>,
        from: usize,
        element: &XHtmlElement<'html>,
    ) -> usize {
        let new_element: Element<'html, 'query> = Element {
            name: to_str!(element.name),
            class: if element.class.is_some() {
                Some(to_str!(element.class.unwrap()))
            } else {
                None
            },
            id: if element.id.is_some() {
                Some(to_str!(element.id.unwrap()))
            } else {
                None
            },
            inner_html: None,
            text_content: None,
            attributes: {
                if element.attributes.is_empty() {
                    NULL_RANGE
                } else {
                    let start = self.attributes.len() as u32;
                    self.attributes.extend(element.attributes.clone());
                    let end = self.attributes.len() as u32;
                    start..end
                }
            },
            children: Vec::new(),
        };

        // attache new element to from element
        // from.children.insert(k, v)
        //println!("Element: {from_element:?}");

        assert!(!self.elements.is_empty());
        assert!(from < self.elements.len());

        let index = self.elements.len();
        self.elements.push(new_element);

        let element = &mut self.elements[from];

        let key_index = element.index_of_child_with_key(selection.source);

        if key_index.is_some() {
            match selection.kind {
                SelectionKind::First(_) => {
                    dbg_print!("Store: {:#?}", self);
                    panic!(
                        "It is not possible to add a single item to the store when it already exists."
                    )
                }
                SelectionKind::All(_) => {
                    let child_index = &mut element.children[key_index.unwrap()].index;
                    match child_index {
                        ChildIndex::One(_) => unreachable!(),
                        ChildIndex::Many(list) => {
                            list.push(index);
                        }
                    }
                    return index;
                }
            }
        }

        element.children.push(Child {
            query: selection.source,
            index: match selection.kind {
                SelectionKind::First(_) => ChildIndex::One(index),
                SelectionKind::All(_) => ChildIndex::Many(vec![index]),
            },
        });

        index
    }

    pub fn set_content(
        &mut self,
        element: usize,
        inner_html: Option<&'html str>,
        text_content_from: Option<usize>,
    ) {
        assert!(!self.elements.is_empty());
        assert!(element < self.elements.len());

        let ele = &mut self.elements[element];
        ele.inner_html = inner_html;
        ele.text_content = if let Some(from) = text_content_from {
            Some((from as u32)..(self.text_content.get_position() as u32))
        } else {
            None
        };
    }
}
/*
#[cfg(test)]
mod tests {

    use crate::runner::element::XHtmlElement;
    use crate::{css::Save, css::SelectionPart, utils::Reader};

    use super::*;

    #[test]
    fn test_element_access() -> Result<(), QueryError<'static>> {
        // Build a tree
        let mut store = RustStore::new(());

        let title_elem = XHtmlElement {
            closing: false,
            name: b"h1",
            id: None,
            class: None,
            attributes: vec![],
        };

        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let title_idx = store.push(&sel_title, crate::store::ROOT, title_elem);
        store.set_content(title_idx, None, Some("Hello".to_string()));

        let sel_items = SelectionPart::new(
            "items",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();

        let li1_elem = XHtmlElement {
            closing: false,
            name: b"li",
            id: None,
            class: None,
            attributes: vec![],
        };
        let li1_idx = store.push(&sel_items, crate::store::ROOT, li1_elem);
        store.set_content(li1_idx, None, Some("First".to_string()));

        let li2_elem = XHtmlElement {
            closing: false,
            name: b"li",
            id: None,
            class: None,
            attributes: vec![],
        };
        let li2_idx = store.push(&sel_items, crate::store::ROOT, li2_elem);
        store.set_content(li2_idx, None, Some("Second".to_string()));

        let doc = &store.elements[0];

        // Different ways of acessing fields
        assert_eq!(store.elements[doc.get("title")?.value()?].name, "h1");
        assert_eq!(store.elements[doc["title"].value()?].name, "h1");

        assert_eq!(doc.get("items")?.iter()?.count(), 2);
        assert_eq!(doc["items"].iter()?.count(), 2);

        // doc.get("items")?.get(0) equivalent
        let first_idx = doc.get("items")?.iter()?.next().unwrap();
        assert_eq!(
            store.elements[*first_idx].text_content,
            Some(&["First"])
        );
        assert_eq!(
            store.elements[doc["items"][0]].text_content,
            Some("First")
        );

        // Iterators for All Selections
        let items_iter1 = doc.get("items")?.iter()?;
        let collected1: Vec<&Element> = items_iter1.map(|&i| &store.elements[i]).collect();
        assert_eq!(collected1.len(), 2);
        assert_eq!(collected1[0].text_content, Some("First"));
        assert_eq!(collected1[1].text_content, Some("Second"));

        let items_iter2 = doc["items"].iter()?;
        let collected2: Vec<&Element> = items_iter2.map(|&i| &store.elements[i]).collect();
        assert_eq!(collected2.len(), 2);
        assert_eq!(collected2[0].text_content, Some("First".to_string()));
        assert_eq!(collected2[1].text_content, Some("Second".to_string()));

        assert!(doc.index_of_child_with_key("optional").is_none());

        assert_eq!(
            doc.get("optional"),
            Err(QueryError::KeyNotFound("optional"))
        );

        Ok(())
    }

    #[test]
    #[should_panic(expected = "no entry found for key")]
    fn test_non_existing_key_element_access() {
        let mut store = RustStore::new(());
        let title_elem = XHtmlElement {
            closing: false,
            name: b"h1",
            id: None,
            class: None,
            attributes: vec![],
        };
        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let _ = store.push(&sel_title, crate::store::ROOT, title_elem);

        let doc = &store.elements[0];

        assert!(doc.index_of_child_with_key("non-existing").is_none());
        assert_eq!(
            doc.get("non-existing"),
            Err(QueryError::KeyNotFound("non-existing"))
        );

        let _ = &doc["non-existing"];
    }

    #[test]
    #[should_panic(expected = "Cannot use usize index on single element")]
    fn test_index_on_single_element_access() {
        let mut store = RustStore::new(());
        let title_elem = XHtmlElement {
            closing: false,
            name: b"h1",
            id: None,
            class: None,
            attributes: vec![],
        };
        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let _ = store.push(&sel_title, crate::store::ROOT, title_elem);

        let doc = &store.elements[0];

        assert!(doc.get("title").unwrap().iter().is_err());

        let _ = doc["title"][0];
    }

    #[test]
    fn test_build_tree() {
        /*
         * root -> a.class --> span#id --> p
         *      |          \-> p
         *      \-> div
         */
        let mut store = RustStore::new(());

        // SETUP Elements
        let first = XHtmlElement {
            closing: false,
            name: b"a",
            id: None,
            class: Some(b"class"),
            attributes: vec![],
        };

        let second = XHtmlElement {
            closing: false,
            name: b"span",
            id: Some(b"id"),
            class: None,
            attributes: vec![],
        };

        let second_extended = XHtmlElement {
            closing: false,
            name: b"p",
            id: None,
            class: None,
            attributes: vec![],
        };

        let second_alternate = XHtmlElement {
            closing: false,
            name: b"p",
            id: None,
            class: None,
            attributes: vec![],
        };

        let first_alternate = XHtmlElement {
            closing: false,
            name: b"div",
            id: None,
            class: None,
            attributes: vec![],
        };

        let selection_first = SelectionPart::new(
            "a",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        )
        .build();

        let selection_first_alternate = SelectionPart::new(
            "div",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        )
        .build();

        let selection_second_alternate = SelectionPart::new(
            "p",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        )
        .build();

        let selection_second = SelectionPart::new(
            "span",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        )
        .build();

        let selection_second_extended = SelectionPart::new(
            "p",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        )
        .build();

        println!("Store: {:#?}", store);

        let mut last_element = store.push(&selection_first, crate::store::ROOT, first);
        let _ = store.push(
            &selection_first_alternate,
            crate::store::ROOT,
            first_alternate,
        );
        let _ = store.push(&selection_second_alternate, last_element, second_alternate);
        last_element = store.push(&selection_second, last_element, second);
        let _ = store.push(&selection_second_extended, last_element, second_extended);

        /*
         * root -> a.class --> span#id --> p
         *      |          \-> p
         *      \-> div
         */
        assert_eq!(
            store.elements[0],
            Element {
                name: "root",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![
                    Child {
                        query: "a",
                        index: ChildIndex::Many(vec![1])
                    },
                    Child {
                        query: "div",
                        index: ChildIndex::Many(vec![2])
                    },
                ]
            }
        );

        assert_eq!(
            store.elements[1],
            Element {
                name: "a",
                id: None,
                class: Some("class"),
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![
                    Child {
                        query: "p",
                        index: ChildIndex::Many(vec![3])
                    },
                    Child {
                        query: "span",
                        index: ChildIndex::Many(vec![4])
                    },
                ]
            }
        );

        assert_eq!(
            store.elements[2],
            Element {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![]
            }
        );

        assert_eq!(
            store.elements[3],
            Element {
                name: "p",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![]
            }
        );

        assert_eq!(
            store.elements[4],
            Element {
                name: "span",
                id: Some("id"),
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![Child {
                    query: "p",
                    index: ChildIndex::Many(vec![5])
                }]
            }
        );

        assert_eq!(
            store.elements[5],
            Element {
                name: "p",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![]
            }
        );
    }
}
 */
