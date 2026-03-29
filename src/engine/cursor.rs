use std::fmt::Debug;

use crate::XHtmlElement;
use crate::query::compiler::{Position, Query};
use smallvec::SmallVec;

use crate::store::ElementId;

#[derive(PartialEq, Debug)]
pub struct Cursor {
    pub(super) parent: ElementId,
    pub(super) position: Position,
    pub(super) match_stack: SmallVec<[super::DepthSize; 10]>,
    pub(super) end: bool, // This is a flag to say is a save point and this might be the end
}

pub trait CursorOps<'query, 'html> {
    fn next(
        &self,
        tree: &Query<'query>,
        depth: super::DepthSize,
        element: &XHtmlElement<'html>,
    ) -> bool;
    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &'html str) -> bool;
    fn step_backward(&mut self, tree: &Query<'query>);

    fn get_position(&self) -> &Position;
    fn set_position(&mut self, value: Position);
    fn set_state(&mut self, value: usize);

    fn get_parent(&self) -> ElementId;
    fn set_parent(&mut self, value: ElementId);

    fn set_end(&mut self, end: bool);

    fn add_depth(&mut self, depth: super::DepthSize);
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            parent: ElementId::default(),
            position: Position {
                selection: 0,
                state: 0,
            },
            match_stack: SmallVec::new(),
            end: false,
        }
    }
}

impl<'query, 'html> CursorOps<'query, 'html> for Cursor {
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get_transition(self.position.state);
        let last_depth = *self.match_stack.last().unwrap_or(&0);
        fsm.next(element, depth, last_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get_transition(self.position.state);
        let last_depth = *self.match_stack.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    fn step_backward(&mut self, tree: &Query<'query>) {
        self.match_stack.pop();

        self.position.back(tree);
    }

    fn get_position(&self) -> &Position {
        &self.position
    }

    fn set_position(&mut self, value: Position) {
        self.position = value;
    }

    fn set_state(&mut self, value: usize) {
        self.position.state = value;
    }

    fn get_parent(&self) -> ElementId {
        self.parent
    }

    fn set_parent(&mut self, value: ElementId) {
        self.parent = value;
    }

    fn set_end(&mut self, end: bool) {
        self.end = end;
    }

    fn add_depth(&mut self, depth: super::DepthSize) {
        self.match_stack.push(depth);
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ScopedCursor {
    pub scope_depth: super::DepthSize,
    pub parent: ElementId,
    pub position: Position,
}

impl ScopedCursor {
    pub fn new(scope_depth: super::DepthSize, parent: ElementId, position: Position) -> Self {
        Self {
            scope_depth,
            parent,
            position,
        }
    }
}

impl<'query, 'html> CursorOps<'query, 'html> for ScopedCursor {
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get_transition(self.position.state);
        fsm.next(element, depth, self.scope_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get_transition(self.position.state);
        fsm.back(element, depth, self.scope_depth)
    }

    fn get_parent(&self) -> ElementId {
        self.parent
    }

    fn set_parent(&mut self, value: ElementId) {
        self.parent = value;
    }

    fn get_position(&self) -> &Position {
        &self.position
    }

    fn set_position(&mut self, value: Position) {
        self.position = value;
    }

    fn set_state(&mut self, value: usize) {
        self.position.state = value;
    }

    fn add_depth(&mut self, _depth: super::DepthSize) {}
    fn step_backward(&mut self, _tree: &Query<'query>) {}
    fn set_end(&mut self, _: bool) {}
}

#[cfg(test)]
mod tests {
    use super::{Cursor, CursorOps};
    use crate::Query;
    use crate::html::element::builder::XHtmlElement;
    use crate::query::compiler::Save;

    #[test]
    fn test_fsm_next_descendant() {
        let query = Query::all("div a", Save::none()).build();

        let mut state = Cursor::new();
        let mut next = state.next(
            &query,
            0,
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: &[],
            },
        );

        assert!(next);

        // move task
        //state.step_foward(&query, 0);
        let position = state.position.next_transition(&query);
        state.position.state = position.unwrap();

        next = state.next(
            &query,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: &[],
            },
        );

        assert!(next);
    }
}
