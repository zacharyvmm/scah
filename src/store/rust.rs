use crate::css::{SelectionKind, SelectionPart};

use super::header::{QueryError, Store};
use crate::mut_prt_unchecked;
use std::collections::HashMap;
use std::ops::Index;
use std::ptr;

#[derive(Debug, PartialEq)]
pub enum SelectionValue<'html, 'query> {
    One(Box<Element<'html, 'query>>),
    Many(Vec<Element<'html, 'query>>),
}

impl<'html, 'query: 'html, 'key> SelectionValue<'html, 'query> {
    fn one(&self) -> Result<&Element<'html, 'query>, QueryError<'key>> {
        match self {
            SelectionValue::One(el) => Ok(el),
            SelectionValue::Many(_) => Err(QueryError::NotASingleElement),
        }
    }

    fn list(&'html self) -> Result<&'html [Element<'html, 'query>], QueryError<'key>> {
        match self {
            SelectionValue::Many(vec) => Ok(vec),
            SelectionValue::One(_) => Err(QueryError::NotAList),
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
        match self {
            SelectionValue::One(_) => Err(QueryError::NotAList),
            SelectionValue::Many(vec) => {
                let index = vec.len() - 1;
                vec.push(element);
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
    pub text_content: Option<&'html str>,
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
        match self {
            SelectionValue::Many(vec) => &vec[index], // Panics if out of bounds
            SelectionValue::One(_) => panic!("Cannot use usize index on single element"),
        }
    }
}

impl<'html, 'query> Index<&'query str> for SelectionValue<'html, 'query> {
    type Output = SelectionValue<'html, 'query>;

    fn index(&self, key: &'query str) -> &Self::Output {
        match self {
            SelectionValue::One(boxed_el) => &boxed_el[key], // Panics if key not found
            SelectionValue::Many(_) => panic!("Cannot chain string index on list selection"),
        }
    }
}

pub struct ElementBuilder<'html, 'query> {
    name: &'html str,
    class: Option<&'html str>,
    id: Option<&'html str>,
    attributes: Vec<(&'html str, Option<&'html str>)>,
    inner_html: Option<&'html str>,
    text_content: Option<&'html str>,
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

    pub fn text_content(mut self, text: &'html str) -> Self {
        self.text_content = Some(text);
        self
    }

    /// Add a single child
    pub fn child(mut self, key: &'query str, element: Element<'html, 'query>) -> Self {
        self.children
            .insert(key, SelectionValue::One(Box::new(element)));
        self
    }

    /// Add multiple children
    pub fn children(mut self, key: &'query str, elements: Vec<Element<'html, 'query>>) -> Self {
        self.children.insert(key, SelectionValue::Many(elements));
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
    root: Box<Element<'html, 'query>>,
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
        let children_map = &mut from_element.children;

        if children_map.contains_key(selection.source) {
            match selection.kind {
                SelectionKind::First(_) => panic!(
                    "It is not possible to add a single item to the store when it already exists."
                ),
                SelectionKind::All(_) => {
                    assert!(matches!(
                        children_map[selection.source],
                        SelectionValue::Many(..)
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
                SelectionKind::First(_) => SelectionValue::One(Box::new(new_element)),
                SelectionKind::All(_) => SelectionValue::Many(vec![new_element]),
            },
        );

        return match &children_map[selection.source] {
            SelectionValue::One(element_box) => {
                let mut_element: *mut Element<'html, 'query> =
                    mut_prt_unchecked!(element_box.as_ref());
                return Ok(mut_element);
            }
            SelectionValue::Many(vec) => {
                let index = vec.len() - 1;
                let last_element = &vec[index];

                let mut_element = mut_prt_unchecked!(last_element);
                Ok(mut_element)
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_access() -> Result<(), QueryError<'static>> {
        // Build a tree
        let doc = ElementBuilder::new("html")
            .child(
                "title",
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        // Different ways of acessing fields
        assert_eq!(doc.get("title")?.value()?.name, "h1");
        assert_eq!(doc["title"].value()?.name, "h1");

        assert_eq!(doc.get("items")?.len()?, 2);
        assert_eq!(doc["items"].len()?, 2);

        assert_eq!(doc.get("items")?.get(0)?.text_content, Some("First"));
        assert_eq!(doc["items"][0].text_content, Some("First"));

        // Iterators for All Selections
        let items_iter1 = doc.get("items")?.iter()?;
        assert_eq!(
            items_iter1.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First").build(),
                &ElementBuilder::new("li").text_content("Second").build(),
            ]
        );

        let items_iter2 = doc["items"].iter()?;
        assert_eq!(
            items_iter2.collect::<Vec<&Element>>(),
            vec![
                &ElementBuilder::new("li").text_content("First").build(),
                &ElementBuilder::new("li").text_content("Second").build(),
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
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
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
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
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
                ElementBuilder::new("h1").text_content("Hello").build(),
            )
            .children(
                "items",
                vec![
                    ElementBuilder::new("li").text_content("First").build(),
                    ElementBuilder::new("li").text_content("Second").build(),
                ],
            )
            .build();

        // No Safe equivalent because of the types

        let non_existing_key = &doc["items"]["li"];
    }
}
