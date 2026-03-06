use crate::css::selector::Selection;
use crate::{Attribute, dbg_print, mut_prt_unchecked};
use std::ops::Range;

mod text_content;
pub(crate) use text_content::TextContent;

const NULL: usize = usize::MAX;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct QueryId(pub(crate) usize);

impl QueryId {
    pub fn is_null(&self) -> bool {
        self.0 == NULL
    }
}
impl Default for QueryId {
    fn default() -> Self {
        Self(NULL)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ElementId(pub(crate) usize);

impl ElementId {
    pub fn is_null(&self) -> bool {
        self.0 == NULL
    }
}
impl Default for ElementId {
    fn default() -> Self {
        Self(NULL)
    }
}

#[derive(Debug, PartialEq)]
pub struct Children {
    first_element: ElementId,

    // If you have 50k elements under this (select all links for example)
    // having this extra field avoids traversing throught all of the siblings
    // just to append another Element
    last_element: ElementId,
}

#[derive(Debug, PartialEq, Default)]
pub struct QueryNode<'query> {
    pub query: &'query str,
    pub next_sibling: Option<QueryId>,
    pub children: Option<Children>,
}

#[derive(Default, Debug, PartialEq)]
pub struct Element<'html> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub inner_html: Option<&'html str>,
    text_content: Option<Range<usize>>,
    attributes: Option<Range<u32>>,

    pub first_child_query: Option<QueryId>,
    pub next_sibling: Option<ElementId>,
}

struct ElementIterator<'html> {
    list: &'html [Element<'html>],
    current_element: ElementId,
}

impl<'html> Iterator for ElementIterator<'html> {
    type Item = &'html Element<'html>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_element.is_null() {
            return None;
        }
        let element = &self.list[self.current_element.0];
        self.current_element = match element.next_sibling {
            Some(sibling) => sibling,
            None => ElementId::default(),
        };
        Some(element)
    }
}

fn find_index<T>(item: &T, list: &[T]) -> usize {
    let list_ptr_range = list.as_ptr_range();
    let ptr = std::ptr::from_ref(item);
    assert!(list_ptr_range.contains(&ptr));

    let index = unsafe { ptr.offset_from_unsigned(list_ptr_range.start) };
    index
}

impl<'html> Element<'html> {
    fn iter(
        &self,
        dom: &'html Store<'html, '_>,
    ) -> impl Iterator<Item = &'html Element<'html>> {
        let index = find_index(self, dom.elements.as_ref());

        ElementIterator {
            list: dom.elements.as_ref(),
            current_element: ElementId(index),
        }
    }

    pub fn get(
        &self,
        dom: &'html Store,
        key: &str,
    ) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        let first_query_id = self.first_child_query;
        first_query_id
            .and_then(|id| dom.find_query_sibling(id, key))
            .and_then(|id| dom.queries[id.0].children.as_ref().map(|c| c.first_element))
            .map(|element_id| dom.elements[element_id.0].iter(dom))
    }

    pub fn attributes(&self, dom: &'html Store) -> Option<&'html [Attribute<'html>]> {
        self.attributes
            .as_ref()
            .map(|range| &dom.attributes[(range.start as usize)..(range.end as usize)])
    }
    pub fn attribute(&self, dom: &'html Store, key: &str) -> Option<&'html str> {
        self.attributes.as_ref().and_then(|range| {
            dom.attributes[(range.start as usize)..(range.end as usize)]
                .iter()
                .find(|attr| attr.key == key)
                .and_then(|kv| kv.value)
        })
    }
    pub fn text_content(&self, dom: &'html Store) -> Option<&'html str> {
        self.text_content
            .as_ref()
            .map(|range| dom.text_content.slice(range.clone()))
    }
}

#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    pub elements: Vec<Element<'html>>,
    pub attributes: Vec<Attribute<'html>>,
    pub queries: Vec<QueryNode<'query>>,
    pub text_content: TextContent,
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    pub fn new() -> Self {
        Self {
            elements: vec![],
            queries: vec![],
            text_content: TextContent::new(),
            attributes: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            elements: Vec::with_capacity(capacity / 3),
            queries: vec![],
            text_content: TextContent::with_capacity(capacity / 3),
            attributes: Vec::with_capacity(capacity / 3),
        }
    }

    fn find_query_sibling(&self, id: QueryId, query: &'query str) -> Option<QueryId> {
        let mut id = id;
        loop {
            let query_node = &self.queries[id.0];
            if query == query_node.query {
                return Some(id);
            }
            match query_node.next_sibling {
                Some(sibling) => id = sibling,
                None => return None,
            }
        }
    }

    pub fn get(&'html self, query: &str) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        if self.queries.is_empty() {
            return None;
        }

        self.find_query_sibling(QueryId(0), query)
            .and_then(|id| {
                self.queries[id.0]
                    .children
                    .as_ref()
                    .map(|c| c.first_element)
            })
            .map(|element_id| self.elements[element_id.0].iter(self))
    }

    fn attribute_slice_to_range(&self, attributes: &[Attribute<'html>]) -> Option<Range<u32>> {
        if self.attributes.is_empty() || attributes.is_empty() {
            return None;
        }
        let tape_pointer_range = self.attributes.as_ptr_range();
        let slice_ptr = attributes.as_ptr();

        // println!("Tape: {:#?}", tape_pointer_range);
        // println!("Slice Pointer: {:#?}", slice_ptr);

        assert!(
            tape_pointer_range.start == slice_ptr || tape_pointer_range.contains(&slice_ptr),
            "Attribute Slice is invalid"
        );

        let start = unsafe { slice_ptr.offset_from_unsigned(tape_pointer_range.start) };
        let end = start + attributes.len();
        assert!(self.attributes.len() >= end);

        Some(std::ops::Range {
            start: start as u32,
            end: end as u32,
        })
    }

    pub fn push(
        &mut self,
        from: ElementId,
        selection: &Selection<'query>,
        element: crate::XHtmlElement<'html>,
    ) -> ElementId {
        let new_element = Element {
            name: element.name,
            class: element.class,
            id: element.id,
            attributes: self.attribute_slice_to_range(element.attributes),
            ..Default::default()
        };

        assert!(from.is_null() || from.0 < self.elements.len());

        let existing_id = {
            if !from.is_null() {
                self.elements[from.0]
                    .first_child_query
                    .and_then(|query| self.find_query_sibling(query, selection.source))
            } else if !self.queries.is_empty() {
                self.find_query_sibling(QueryId(0), selection.source)
            } else {
                None
            }
        };
        let query_id = match existing_id {
            Some(id) => id,
            None => {
                self.queries.push(QueryNode {
                    query: selection.source,
                    ..Default::default()
                });
                let new_id = QueryId(self.queries.len() - 1);

                if !from.is_null() && let Some(last) = self.elements[from.0].iter(self).last(){
                    // I can't be asked to create a function that specificly get the last Element as mut ref Right Now
                    let last = unsafe{mut_prt_unchecked!(last).as_mut()}.unwrap();
                    assert!(last.first_child_query.is_none());
                    last.first_child_query = Some(new_id);
                }


                new_id
            }
        };

        assert!(!self.queries.is_empty());
        assert!(query_id.0 < self.queries.len());

        let index = ElementId(self.elements.len());
        self.elements.push(new_element);

        let query = &mut self.queries[query_id.0];

        match &mut query.children {
            Some(children) => {
                let element = &mut self.elements[children.last_element.0];
                assert_eq!(
                    element.next_sibling, None,
                    "The `last_element` of a query should not have another sibling"
                );

                element.next_sibling = Some(index);
                children.last_element = index;
            }
            None => {
                query.children = Some(Children {
                    first_element: index,
                    last_element: index,
                });

                if !from.is_null() {
                    // TODO: this is redoing the work from `find_query_sibling`. 
                    let element = &mut self.elements[from.0];
                    let last_query = element.first_child_query.and_then(|mut sibling| {
                        loop {
                            let query_node = &self.queries[sibling.0];
                            match query_node.next_sibling {
                                Some(next) => sibling = next,
                                None => break,
                            }
                        }
                        if sibling == query_id {
                            None
                        } else {
                            Some(sibling)
                        }
                    });
                    match last_query {
                        Some(last) => {
                            assert!(self.queries[last.0].next_sibling.is_none());
                            self.queries[last.0].next_sibling = Some(query_id);
                        },
                        None => {
                            assert!(element.first_child_query.is_none());
                            element.first_child_query = Some(query_id);
                        }
                    }
                }
            }
        }

        index
    }

    pub fn set_content<'key>(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        text_content: Option<Range<usize>>,
    ) {
        assert!(!self.elements.is_empty());
        assert!(element_id.0 < self.elements.len());

        let element = &mut self.elements[element_id.0];
        element.inner_html = inner_html;
        element.text_content = text_content;
    }
}

#[cfg(test)]
mod tests {
    use crate::{Query, Save};

    use super::*;

    #[test]
    fn test_find_next_query() {
        let mut store = Store::new();
        store.queries = vec![
            QueryNode {
                query: "1",
                next_sibling: Some(QueryId(1)),
                children: None,
            },
            QueryNode {
                query: "2",
                next_sibling: Some(QueryId(2)),
                children: None,
            },
            QueryNode {
                query: "3",
                next_sibling: Some(QueryId(3)),
                children: None,
            },
            QueryNode {
                // Shouldn't be possible, but still a giid test
                query: "3",
                next_sibling: None,
                children: None,
            },
        ];

        assert_eq!(store.find_query_sibling(QueryId(0), "1"), Some(QueryId(0)));
        assert_eq!(store.find_query_sibling(QueryId(0), "2"), Some(QueryId(1)));
        assert_eq!(store.find_query_sibling(QueryId(0), "3"), Some(QueryId(2)));
        assert_eq!(store.find_query_sibling(QueryId(0), "no in list"), None);
    }

    #[test]
    fn test_push_multi_section() {
        let query = Query::all("main > section", Save::all())
            .then(|section| {
                [
                    section.all("> a[href]", Save::all()),
                    section.all("div a", Save::all()),
                ]
            })
            .build();

        let mut store = Store::new();

        store.push(
            ElementId::default(),
            &query.queries[0],
            crate::XHtmlElement {
                name: "section",
                ..Default::default()
            },
        );

        assert_eq!(
            store.find_query_sibling(QueryId(0), query.queries[0].source),
            Some(QueryId(0))
        );

        store.push(
            ElementId::default(),
            &query.queries[0],
            crate::XHtmlElement {
                name: "section",
                ..Default::default()
            },
        );

        assert_eq!(
            store.elements,
            vec![
                Element {
                    name: "section",
                    next_sibling: Some(ElementId(1)),
                    ..Default::default()
                },
                Element {
                    name: "section",
                    ..Default::default()
                },
            ]
        );

        assert_eq!(
            store.queries,
            vec![QueryNode {
                query: "main > section",
                next_sibling: None,
                children: Some(Children {
                    first_element: ElementId(0),
                    last_element: ElementId(1),
                },),
            },]
        );

        store.push(
            ElementId(1),
            &query.queries[1],
            crate::XHtmlElement {
                name: "a",
                ..Default::default()
            },
        );

        assert_eq!(
            store.queries,
            vec![QueryNode {
                query: "main > section",
                next_sibling: None,
                children: Some(Children {
                    first_element: ElementId(0),
                    last_element: ElementId(1),
                },),
            },QueryNode {
                query: "> a[href]",
                next_sibling: None,
                children: Some(Children {
                    first_element: ElementId(2),
                    last_element: ElementId(2),
                },),
            }]
        );

        assert_eq!(
            store.elements,
            vec![
                Element {
                    name: "section",
                    next_sibling: Some(ElementId(1)),
                    ..Default::default()
                },
                Element {
                    name: "section",
                    first_child_query: Some(QueryId(1)),
                    ..Default::default()
                },
                Element {
                    name: "a",
                    ..Default::default()
                },
            ]
        );
    }
}
