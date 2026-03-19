use std::ops::{Index, IndexMut};

use super::element::ElementId;
use super::iterator::QueryIterator;
use super::span::Span;

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

#[derive(Debug, PartialEq, Default)]
pub struct QueryNode<'query> {
    pub query: &'query str,
    pub next_sibling: Option<QueryId>,
    pub elements: Span<ElementId>,
}

impl<'query> QueryNode<'query> {
    pub fn iter(
        &self,
        arena: &'query QueryArena<'query>,
    ) -> impl Iterator<Item = &'query QueryNode<'query>> {
        QueryIterator::new(self, arena)
    }
}

#[derive(Debug, PartialEq)]
pub struct QueryArena<'query> {
    pub(super) inner: Vec<QueryNode<'query>>,
}

impl<'query> QueryArena<'query> {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, query: QueryNode<'query>) {
        self.inner.push(query)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // This is a function with the goal of find the correct query to add children to or append a query at the end
    pub(super) fn find_query_sibling(&self, id: QueryId, query: &'query str) -> Option<QueryId> {
        let mut id = id;

        if self[id].query == query {
            return Some(id);
        }

        while let Some(sibling) = self[id].next_sibling {
            id = sibling;

            if self[id].query == query {
                return Some(id);
            }
        }

        None
    }
}

impl<'query> Index<QueryId> for QueryArena<'query> {
    type Output = QueryNode<'query>;
    fn index(&self, index: QueryId) -> &Self::Output {
        &self.inner[index.0]
    }
}

impl<'query> IndexMut<QueryId> for QueryArena<'query> {
    fn index_mut(&mut self, index: QueryId) -> &mut Self::Output {
        &mut self.inner[index.0]
    }
}
