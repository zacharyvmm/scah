use std::ops::{Index, IndexMut, Range};

use super::query::QueryId;
use super::{Attribute, Store};

use super::iterator::ElementIterator;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ElementId(pub(crate) usize);

const NULL: usize = usize::MAX;

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

#[derive(Default, Debug, PartialEq)]
pub struct Element<'html> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub inner_html: Option<&'html str>,
    pub(super) text_content: Option<Range<usize>>,
    pub(super) attributes: Option<Range<u32>>,

    pub first_child_query: Option<QueryId>,
    pub next_sibling: Option<ElementId>,
}

impl<'html> Element<'html> {
    pub fn iter(
        &self,
        arena: &'html ElementArena<'html>,
    ) -> impl Iterator<Item = &'html Element<'html>> {
        ElementIterator::new(self, arena)
    }

    pub fn get(
        &self,
        dom: &'html Store,
        key: &str,
    ) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        let first_query_id = self.first_child_query;
        first_query_id
            .and_then(|id| dom.queries.find_query_sibling(id, key))
            .map(|id| dom.queries[id].elements.start())
            .map(|element_id| dom.elements[element_id].iter(&dom.elements))
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
pub struct ElementArena<'html> {
    pub(super) inner: Vec<Element<'html>>,
}

impl<'html> ElementArena<'html> {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, element: Element<'html>) {
        self.inner.push(element)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<'html> Index<ElementId> for ElementArena<'html> {
    type Output = Element<'html>;
    fn index(&self, index: ElementId) -> &Self::Output {
        &self.inner[index.0]
    }
}

impl<'html> IndexMut<ElementId> for ElementArena<'html> {
    fn index_mut(&mut self, index: ElementId) -> &mut Self::Output {
        &mut self.inner[index.0]
    }
}
