use crate::css::SelectionKind;
use crate::xhtml::element::element::Attributes;

use crate::xhtml::text_content::TextContent;
use crate::{QuerySection, dbg_print};
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
type StrRange = Range<usize>;

#[derive(Debug, PartialEq)]
pub struct Element<'html, 'query> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub attributes: Attributes<'html>,
    pub inner_html: Option<&'html str>,
    pub text_content: Option<Range<usize>>,
    // Store Selection directly to enable Index trait
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

impl<'html, 'query> Index<&'query str> for Element<'html, 'query> {
    type Output = Child<'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        let index = self
            .index_of_child_with_key(key)
            .expect("no entry found for key");
        &self.children[index]
    }
}

#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    pub elements: Vec<Element<'html, 'query>>,
    pub text_content: TextContent,
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    pub fn new() -> Self {
        Self {
            elements: vec![Element {
                name: "root",
                class: None,
                id: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![],
            }],
            text_content: TextContent::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut arena = Vec::with_capacity(capacity/3);
        arena.push(
            Element {
                name: "root",
                class: None,
                id: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: vec![],
            }
        );
        Self {
            elements: arena,
            text_content: TextContent::with_capacity(capacity/3),
        }
    }

    pub fn text_content(&self, element: &Element<'html, 'query>) -> Option<&str> {
        if let Some(content) = &element.text_content {
            Some(&self.text_content.slice(content.start..content.end))
        } else {
            None
        }
    }

    pub fn push(
        &mut self,
        selection: &QuerySection<'query>,
        from: usize,
        element: crate::XHtmlElement<'html>,
    ) -> usize {
        let new_element: Element<'html, 'query> = Element {
            name: element.name,
            class: element.class,
            id: element.id,
            attributes: element.attributes,
            inner_html: None,
            text_content: None,
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

    pub fn set_content<'key>(
        &mut self,
        element: usize,
        inner_html: Option<&'html str>,
        text_content: Option<Range<usize>>,
    ) {
        assert!(!self.elements.is_empty());
        assert!(element < self.elements.len());

        let ele = &mut self.elements[element];
        ele.inner_html = inner_html;
        ele.text_content = text_content;
    }
}
/*

#[cfg(test)]
mod tests {

    use crate::{XHtmlElement, css::Save, css::SelectionPart, utils::Reader};

    use super::*;

    const ROOT: usize = 0;

    #[test]
    fn test_element_access() -> Result<(), QueryError<'static>> {
        // Build a tree
        let mut store = Store::new();

        let mut title_elem = XHtmlElement::new();
        title_elem.from(&mut Reader::new("h1"));

        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let title_idx = store.push(&sel_title, ROOT, title_elem);
        store.set_content(title_idx, None, Some("Hello".to_string()));

        let sel_items = SelectionPart::new(
            "items",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();

        let mut li1_elem = XHtmlElement::new();
        li1_elem.from(&mut Reader::new("li"));
        let li1_idx = store.push(&sel_items, ROOT, li1_elem);
        store.set_content(li1_idx, None, Some("First".to_string()));

        let mut li2_elem = XHtmlElement::new();
        li2_elem.from(&mut Reader::new("li"));
        let li2_idx = store.push(&sel_items, ROOT, li2_elem);
        store.set_content(li2_idx, None, Some("Second".to_string()));

        let doc = &store.arena[0];

        // Different ways of acessing fields
        assert_eq!(store.arena[doc.get("title")?.value()?].name, "h1");
        assert_eq!(store.arena[doc["title"].value()?].name, "h1");

        assert_eq!(doc.get("items")?.iter()?.count(), 2);
        assert_eq!(doc["items"].iter()?.count(), 2);

        // doc.get("items")?.get(0) equivalent
        let first_idx = doc.get("items")?.iter()?.next().unwrap();
        assert_eq!(
            store.arena[*first_idx].text_content,
            Some("First".to_string())
        );
        assert_eq!(
            store.arena[doc["items"][0]].text_content,
            Some("First".to_string())
        );

        // Iterators for All Selections
        let items_iter1 = doc.get("items")?.iter()?;
        let collected1: Vec<&Element> = items_iter1.map(|&i| &store.arena[i]).collect();
        assert_eq!(collected1.len(), 2);
        assert_eq!(collected1[0].text_content, Some("First".to_string()));
        assert_eq!(collected1[1].text_content, Some("Second".to_string()));

        let items_iter2 = doc["items"].iter()?;
        let collected2: Vec<&Element> = items_iter2.map(|&i| &store.arena[i]).collect();
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
        let mut store = Store::new();
        let mut title_elem = XHtmlElement::new();
        title_elem.from(&mut Reader::new("h1"));
        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let _ = store.push(&sel_title, ROOT, title_elem);

        let doc = &store.arena[0];

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
        let mut store = Store::new();
        let mut title_elem = XHtmlElement::new();
        title_elem.from(&mut Reader::new("h1"));
        let sel_title = SelectionPart::new(
            "title",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: true,
            }),
        )
        .build();
        let _ = store.push(&sel_title, ROOT, title_elem);

        let doc = &store.arena[0];

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
        let mut store = Store::new();

        // SETUP Elements
        let mut first = XHtmlElement::new();
        first.from(&mut Reader::new("a class=\"class\""));

        let mut second = XHtmlElement::new();
        second.from(&mut Reader::new("span id=\"id\""));

        let mut second_extended = XHtmlElement::new();
        second_extended.from(&mut Reader::new("p"));

        let mut second_alternate = XHtmlElement::new();
        second_alternate.from(&mut Reader::new("p"));

        let mut first_alternate = XHtmlElement::new();
        first_alternate.from(&mut Reader::new("div"));

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

        let mut last_element = store.push(&selection_first, ROOT, first);
        let _ = store.push(
            &selection_first_alternate,
            ROOT,
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
            store.arena[0],
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
            store.arena[1],
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
            store.arena[2],
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
            store.arena[3],
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
            store.arena[4],
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
            store.arena[5],
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
