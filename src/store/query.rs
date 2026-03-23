use super::arena::Arena;
use super::arena::span::Span;
use super::arena::{Node, id};

#[derive(Debug, PartialEq, Default)]
pub struct QueryNode<'query> {
    pub query: &'query str,
    pub next_sibling: Option<id::QueryId>,
    pub elements: Span<id::ElementId>,
}

impl<'query> Node<id::QueryId> for QueryNode<'query> {
    fn next_sibling(&self) -> Option<id::QueryId> {
        self.next_sibling
    }
}

impl<'query> QueryNode<'query> {
    pub fn iter(
        &self,
        arena: &'query Arena<QueryNode<'query>, id::QueryId>,
    ) -> impl Iterator<Item = &'query QueryNode<'query>> {
        let index = unsafe { arena.index_of(self) };
        arena.iter_from(index)
    }
}
