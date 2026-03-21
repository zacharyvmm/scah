use std::ops::{Deref, Range};

use super::arena::{Arena, Node, id};
use super::{Attribute, Store};

#[derive(Default, Debug, PartialEq)]
pub struct Element<'html> {
    pub name: &'html str,
    pub class: Option<&'html str>,
    pub id: Option<&'html str>,
    pub inner_html: Option<&'html str>,
    pub(super) text_content: Option<Range<usize>>,
    pub(super) attributes: Option<Range<u32>>,

    pub first_child_query: Option<id::QueryId>,
    pub next_sibling: Option<id::ElementId>,
}

impl<'html> Node<id::ElementId> for Element<'html> {
    fn next_sibling(&self) -> Option<id::ElementId> {
        self.next_sibling
    }
}

impl<'html> Element<'html> {
    pub fn iter(
        &self,
        arena: &'html Arena<Element<'html>, id::ElementId>,
    ) -> impl Iterator<Item = &'html Element<'html>> {
        let index = unsafe { arena.index_of(self) };
        arena.iter_from(index)
    }

    pub fn get(
        &self,
        dom: &'html Store,
        key: &str,
    ) -> Option<impl Iterator<Item = &'html Element<'html>>> {
        let first_query_id = self.first_child_query;
        first_query_id
            .and_then(|id| dom.queries.iter_from(id).find(|q| q.query == key))
            .map(|query_node| query_node.elements.start())
            .map(|element_id| dom.elements.iter_from(element_id))
    }

    pub fn attributes(&self, dom: &'html Store) -> Option<&'html [Attribute<'html>]> {
        self.attributes
            .as_ref()
            .map(|range| &dom.attributes.deref()[(range.start as usize)..(range.end as usize)])
    }
    pub fn attribute(&self, dom: &'html Store, key: &str) -> Option<&'html str> {
        self.attributes.as_ref().and_then(|range| {
            dom.attributes.deref()[(range.start as usize)..(range.end as usize)]
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
