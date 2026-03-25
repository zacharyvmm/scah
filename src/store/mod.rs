use crate::css::selector::QuerySection;
use crate::{Attribute, dbg_print, mut_prt_unchecked};
use std::ops::Range;

mod text_content;
pub(crate) use text_content::TextContent;
mod arena;
mod attributes;
mod element;
mod query;

pub(crate) use arena::id::Nullable;
use arena::span::Span;
pub use arena::{
    Arena,
    id::{AttributeId, ElementId, QueryId},
};

pub use element::Element;
pub use query::QueryNode;

/// The result set returned by [`parse`](crate::parse).
///
/// A `Store` is an arena-based container that holds all elements, attributes,
/// and text content captured during parsing. You query it by CSS selector
/// string using [`Store::get`].
///
/// # Example
///
/// ```rust
/// use scah::{Query, Save, parse};
///
/// let html = "<div><a href='x'>Link1</a><a href='y'>Link2</a></div>";
/// let queries = &[Query::all("a", Save::all()).build()];
/// let store = parse(html, queries);
///
/// // Retrieve all matched <a> elements
/// let anchors: Vec<_> = store.get("a").unwrap().collect();
/// assert_eq!(anchors.len(), 2);
///
/// // Access attributes
/// assert_eq!(anchors[0].attribute(&store, "href"), Some("x"));
/// ```
#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    /// Arena of matched elements.
    pub elements: Arena<Element<'html>, ElementId>,
    /// Arena of attributes belonging to matched elements.
    pub attributes: Arena<Attribute<'html>, AttributeId>,
    /// Arena of query nodes that link selectors to their matched elements.
    pub queries: Arena<QueryNode<'query>, QueryId>,
    /// Accumulated text-content buffer shared by all elements.
    pub text_content: TextContent,
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    pub fn new() -> Self {
        Self {
            elements: Arena::new(),
            queries: Arena::new(),
            text_content: TextContent::new(),
            attributes: Arena::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            elements: Arena::with_capacity(capacity / 3),
            queries: Arena::new(),
            text_content: TextContent::with_capacity(capacity / 3),
            attributes: Arena::with_capacity(capacity / 3),
        }
    }

    /// Look up all elements that matched a given CSS selector string.
    ///
    /// The `query` parameter must be the **exact same string** used when
    /// building the [`Query`](crate::Query) (e.g. `"main > section > a[href]"`).
    ///
    /// Returns `None` if no elements were matched by any query, or if
    /// the given selector string was not part of the executed queries.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scah::{Query, Save, parse};
    ///
    /// let html = "<ul><li>A</li><li>B</li></ul>";
    /// let queries = &[Query::all("li", Save::only_text_content()).build()];
    /// let store = parse(html, queries);
    ///
    /// for li in store.get("li").unwrap() {
    ///     println!("{}", li.text_content(&store).unwrap_or_default());
    /// }
    /// ```
    pub fn get(&'html self, query: &str) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        if self.queries.is_empty() {
            return None;
        }

        self.queries
            .iter_from(QueryId(0))
            .find(|q| q.query == query)
            .map(|query_node| query_node.elements.start())
            .map(|element_id| self.elements.iter_from(element_id))
    }

    fn link_query_to_element(&mut self, query: QueryId, element: ElementId) {
        let id = self.elements[element].first_child_query;

        match id {
            Some(mut id) => loop {
                if id == query {
                    return;
                }
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
        let id = self.queries[query].elements.end();

        if id == element {
            return;
        }

        assert!(self.elements[id].next_sibling.is_none());
        self.elements[id].next_sibling = Some(element);
        self.queries[query].elements.set_end(element);
    }

    pub fn push(
        &mut self,
        from: ElementId,
        selection: &QuerySection<'query>,
        element: crate::XHtmlElement<'html>,
    ) -> ElementId {
        let new_element = Element {
            name: element.name,
            class: element.class,
            id: element.id,
            attributes: self.attributes.attribute_slice_to_range(element.attributes),
            ..Default::default()
        };

        assert!(from.is_null() || from.0 < self.elements.len());

        let existing_id = {
            if !from.is_null() {
                self.elements[from].first_child_query.and_then(|query| {
                    self.queries
                        .iter_from(query)
                        .find(|q| q.query == selection.source)
                        .map(|q| unsafe { self.queries.index_of(q) })
                })
            } else if !self.queries.is_empty() {
                self.queries
                    .iter_from(QueryId(0))
                    .find(|q| q.query == selection.source)
                    .map(|q| unsafe { self.queries.index_of(q) })
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
                    elements: Span::new(index),
                    next_sibling: None,
                });
                let new_id = QueryId(self.queries.len() - 1);

                new_id
            }
        };

        assert!(!self.queries.is_empty());
        assert!(query_id.0 < self.queries.len());

        if !from.is_null() {
            self.link_query_to_element(query_id, from);
        }

        self.link_element_to_query(query_id, index);

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
            store
                .queries
                .iter_from(QueryId(0))
                .find(|q| q.query == "1")
                .map(|q| unsafe { store.queries.index_of(q) }),
            Some(QueryId(0))
        );
        assert_eq!(
            store
                .queries
                .iter_from(QueryId(0))
                .find(|q| q.query == "2")
                .map(|q| unsafe { store.queries.index_of(q) }),
            Some(QueryId(1))
        );
        assert_eq!(
            store
                .queries
                .iter_from(QueryId(0))
                .find(|q| q.query == "3")
                .map(|q| unsafe { store.queries.index_of(q) }),
            Some(QueryId(2))
        );
        assert_eq!(
            store
                .queries
                .iter_from(QueryId(0))
                .find(|q| q.query == "not in list")
                .map(|q| unsafe { store.queries.index_of(q) }),
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
                elements: Span::new(ElementId(0))
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
                    elements: Span::new(ElementId(0))
                },
                QueryNode {
                    query: "2",
                    next_sibling: None,
                    elements: Span::new(ElementId(1))
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
                    elements: Span::new(ElementId(0))
                },
                QueryNode {
                    query: "2",
                    next_sibling: Some(QueryId(2)),
                    elements: Span::new(ElementId(1))
                },
                QueryNode {
                    query: "3",
                    next_sibling: None,
                    elements: Span::new(ElementId(2))
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
                .iter_from(QueryId(0))
                .find(|q| q.query == query.queries[0].source)
                .map(|q| unsafe { store.queries.index_of(q) }),
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
                elements: Span::from(ElementId(0), ElementId(1))
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
                    elements: Span::from(ElementId(0), ElementId(1))
                },
                QueryNode {
                    query: "> a[href]",
                    next_sibling: None,
                    elements: Span::new(ElementId(2))
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
