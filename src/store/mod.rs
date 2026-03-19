use crate::css::selector::Selection;
use crate::{Attribute, dbg_print, mut_prt_unchecked};
use std::ops::Range;

mod text_content;
pub(crate) use text_content::TextContent;
mod element;
mod iterator;
mod query;
mod span;

pub use element::{Element, ElementArena, ElementId};
pub use query::{QueryArena, QueryId, QueryNode};

#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    pub elements: ElementArena<'html>,
    pub attributes: Vec<Attribute<'html>>,
    pub queries: QueryArena<'query>,
    pub text_content: TextContent,
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    pub fn new() -> Self {
        Self {
            elements: ElementArena::new(),
            queries: QueryArena::new(),
            text_content: TextContent::new(),
            attributes: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            elements: ElementArena::with_capacity(capacity / 3),
            queries: QueryArena::new(),
            text_content: TextContent::with_capacity(capacity / 3),
            attributes: Vec::with_capacity(capacity / 3),
        }
    }

    pub fn get(&'html self, query: &str) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        if self.queries.is_empty() {
            return None;
        }

        self.queries
            .find_query_sibling(QueryId(0), query)
            .map(|id| self.queries[id].first_element)
            .and_then(|element_id| self.elements.inner.get(element_id.0))
            .map(|element| element.iter(&self.elements))
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

    fn link_query_to_element(&mut self, query: QueryId, element: ElementId) {
        let id = self.elements[element].first_child_query;
        match id {
            Some(mut id) => loop {
                let query_node = &self.queries[id];
                match query_node.next_sibling {
                    Some(sibling) => id = sibling,
                    None => {
                        self.queries[id].next_sibling = Some(query);
                        break;
                    }
                }
            },
            None => {
                self.elements[element].first_child_query = Some(query);
            }
        }
    }

    fn link_element_to_query(&mut self, query: QueryId, element: ElementId) {
        let id = self.queries[query].last_element;

        assert!(self.elements[id].next_sibling.is_none());
        self.elements[id].next_sibling = Some(element);
        self.queries[query].last_element = element;
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
                self.elements[from]
                    .first_child_query
                    .and_then(|query| self.queries.find_query_sibling(query, selection.source))
            } else if !self.queries.is_empty() {
                self.queries
                    .find_query_sibling(QueryId(0), selection.source)
            } else {
                None
            }
        };

        let index = ElementId(self.elements.len());
        self.elements.push(new_element);

        let query_id = match existing_id {
            Some(id) => id,
            None => {
                self.queries.push(QueryNode {
                    query: selection.source,
                    first_element: index,
                    last_element: index,
                    next_sibling: None,
                });
                let new_id = QueryId(self.queries.len() - 1);

                new_id
            }
        };

        assert!(!self.queries.is_empty());
        assert!(query_id.0 < self.queries.len());

        if !from.is_null() {
            self.link_element_to_query(query_id, from);
        }

        //let query = &mut self.queries[query_id.0];

        // let element = &mut self.elements[query.last_element.0];
        // element.next_sibling = Some(index);
        // children.last_element = index;

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

        let element = &mut self.elements[element_id];
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
        store.queries.inner = vec![
            QueryNode {
                query: "1",
                next_sibling: Some(QueryId(1)),
                ..Default::default()
            },
            QueryNode {
                query: "2",
                next_sibling: Some(QueryId(2)),
                ..Default::default()
            },
            QueryNode {
                query: "3",
                next_sibling: Some(QueryId(3)),
                ..Default::default()
            },
            QueryNode {
                // Shouldn't be possible, but still a giid test
                query: "3",
                next_sibling: None,
                ..Default::default()
            },
        ];

        assert_eq!(
            store.queries.find_query_sibling(QueryId(0), "1"),
            Some(QueryId(0))
        );
        assert_eq!(
            store.queries.find_query_sibling(QueryId(0), "2"),
            Some(QueryId(1))
        );
        assert_eq!(
            store.queries.find_query_sibling(QueryId(0), "3"),
            Some(QueryId(2))
        );
        assert_eq!(
            store.queries.find_query_sibling(QueryId(0), "no in list"),
            None
        );
    }

    #[test]
    fn test_link_query_to_element() {
        let mut store = Store::new();

        store.elements.inner = vec![
            Element {
                first_child_query: Some(QueryId(0)),
                ..Default::default()
            },
            Element {
                first_child_query: None,
                ..Default::default()
            },
        ];

        store.queries.inner = vec![
            QueryNode {
                next_sibling: Some(QueryId(1)),
                ..Default::default()
            },
            QueryNode {
                next_sibling: None,
                ..Default::default()
            },
        ];

        store.link_query_to_element(QueryId(0), ElementId(1));

        assert_eq!(
            store.queries.inner,
            vec![
                QueryNode {
                    next_sibling: Some(QueryId(1)),
                    ..Default::default()
                },
                QueryNode {
                    next_sibling: None,
                    // first_element: ElementId(1),
                    // last_element: ElementId(1),
                    ..Default::default()
                }
            ]
        );
    }

    #[test]
    fn test_branching_next_query() {
        let mut store = Store::new();

        let q = Query::all("1", Save::all())
            .then(|ctx| [ctx.all("2", Save::all()), ctx.all("3", Save::all())]);

        // `1` MATCH
        store.push(
            ElementId::default(),
            &q.selection[0],
            crate::XHtmlElement::default(),
        );

        assert_eq!(
            store.queries.inner,
            vec![QueryNode {
                query: "1",
                next_sibling: None,
                first_element: ElementId(0),
                last_element: ElementId(0),
            }]
        );

        assert_eq!(store.elements.inner, vec![Element::default(),]);

        // `2` MATCH
        store.push(
            ElementId(0),
            &q.selection[1],
            crate::XHtmlElement::default(),
        );

        assert_eq!(
            store.queries.inner,
            vec![
                QueryNode {
                    query: "1",
                    next_sibling: None,
                    first_element: ElementId(0),
                    last_element: ElementId(0),
                },
                QueryNode {
                    query: "2",
                    next_sibling: None,
                    first_element: ElementId(1),
                    last_element: ElementId(1),
                }
            ]
        );

        assert_eq!(
            store.elements.inner,
            vec![
                Element {
                    first_child_query: Some(QueryId(1)),
                    ..Default::default()
                },
                Element {
                    ..Default::default()
                },
            ]
        );

        // `3` MATCH
        store.push(
            ElementId(0),
            &q.selection[2],
            crate::XHtmlElement::default(),
        );

        assert_eq!(
            store.queries.inner,
            vec![
                QueryNode {
                    query: "1",
                    next_sibling: None,
                    first_element: ElementId(0),
                    last_element: ElementId(0),
                },
                QueryNode {
                    query: "2",
                    next_sibling: Some(QueryId(2)),
                    first_element: ElementId(1),
                    last_element: ElementId(1),
                },
                QueryNode {
                    query: "3",
                    next_sibling: None,
                    first_element: ElementId(2),
                    last_element: ElementId(2),
                }
            ]
        );

        assert_eq!(
            store.elements.inner,
            vec![
                Element {
                    first_child_query: Some(QueryId(1)),
                    ..Default::default()
                },
                Element {
                    ..Default::default()
                },
                Element {
                    ..Default::default()
                },
            ]
        );
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
            store
                .queries
                .find_query_sibling(QueryId(0), query.queries[0].source),
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
            store.elements.inner,
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
            store.queries.inner,
            vec![QueryNode {
                query: "main > section",
                next_sibling: None,
                first_element: ElementId(0),
                last_element: ElementId(1),
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
            store.queries.inner,
            vec![
                QueryNode {
                    query: "main > section",
                    next_sibling: None,
                    first_element: ElementId(0),
                    last_element: ElementId(1),
                },
                QueryNode {
                    query: "> a[href]",
                    next_sibling: None,
                    first_element: ElementId(2),
                    last_element: ElementId(2),
                }
            ]
        );

        assert_eq!(
            store.elements.inner,
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
