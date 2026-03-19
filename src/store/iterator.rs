use super::element::{Element, ElementArena, ElementId};
use super::query::{QueryArena, QueryId, QueryNode};

pub(super) unsafe fn find_index<T>(item: &T, list: &[T]) -> usize {
    let list_ptr_range = list.as_ptr_range();
    let ptr = std::ptr::from_ref(item);
    assert!(list_ptr_range.contains(&ptr));

    let index = unsafe { ptr.offset_from_unsigned(list_ptr_range.start) };
    index
}

pub(super) struct ElementIterator<'html> {
    arena: &'html ElementArena<'html>,
    current_element: ElementId,
}

impl<'html> ElementIterator<'html> {
    pub(super) fn new(element: &Element, arena: &'html ElementArena<'html>) -> Self {
        let index = unsafe { find_index(element, &arena.inner) };

        Self {
            arena,
            current_element: ElementId(index),
        }
    }
}

impl<'html> Iterator for ElementIterator<'html> {
    type Item = &'html Element<'html>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_element.is_null() {
            return None;
        }
        let element = &self.arena[self.current_element];
        self.current_element = match element.next_sibling {
            Some(sibling) => sibling,
            None => ElementId::default(),
        };
        Some(element)
    }
}

pub(super) struct QueryIterator<'query> {
    arena: &'query QueryArena<'query>,
    cursor: QueryId,
}

impl<'query> QueryIterator<'query> {
    pub(super) fn new(query: &QueryNode, arena: &'query QueryArena<'query>) -> Self {
        let index = unsafe { find_index(query, &arena.inner) };

        Self {
            arena,
            cursor: QueryId(index),
        }
    }
}

impl<'query> Iterator for QueryIterator<'query> {
    type Item = &'query QueryNode<'query>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.is_null() {
            return None;
        }
        let query = &self.arena[self.cursor];
        self.cursor = match query.next_sibling {
            Some(sibling) => sibling,
            None => QueryId::default(),
        };
        Some(query)
    }
}
