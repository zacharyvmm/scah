use std::fmt::Debug;

use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{Position, Query};
use smallvec::SmallVec;

#[derive(PartialEq, Debug)]
pub struct FsmState {
    pub(super) parent: usize,
    pub(super) position: Position,
    pub(super) depths: SmallVec<[super::DepthSize; 10]>,
    pub(super) end: bool, // This is a flag to say is a save point and this might be the end
}

pub trait Fsm<'query, 'html> {
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

    fn get_parent(&self) -> usize;
    fn set_parent(&mut self, value: usize);

    fn set_end(&mut self, end: bool);

    fn add_depth(&mut self, depth: super::DepthSize);
}

impl<'query> FsmState {
    pub fn new() -> Self {
        Self {
            parent: 0,
            position: Position {
                selection: 0,
                state: 0,
            },
            depths: SmallVec::new(),
            end: false,
        }
    }
}

impl<'query, 'html> Fsm<'query, 'html> for FsmState {
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get_state(self.position.state);
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.next(element, depth, last_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get_state(self.position.state);
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    fn step_backward(&mut self, tree: &Query<'query>) {
        // BUG: Currently this works for opening a closing element's, but if in a ALL selection
        // The FSM position and make it break
        self.depths.pop();

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

    fn get_parent(&self) -> usize {
        self.parent
    }

    fn set_parent(&mut self, value: usize) {
        self.parent = value;
    }

    fn set_end(&mut self, end: bool) {
        self.end = end;
    }

    fn add_depth(&mut self, depth: super::DepthSize) {
        self.depths.push(depth);
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ScopedFsm {
    pub scope_depth: super::DepthSize,
    pub parent: usize,
    pub position: Position,
}

impl<'query> ScopedFsm {
    pub fn new(scope_depth: super::DepthSize, parent: usize, position: Position) -> Self {
        Self {
            scope_depth,
            parent,
            position,
        }
    }

    pub fn in_scope(&self, current_depth: super::DepthSize) -> bool {
        self.scope_depth > current_depth
    }
}

impl<'query, 'html> Fsm<'query, 'html> for ScopedFsm {
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get_state(self.position.state);
        fsm.next(element, depth, self.scope_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get_state(self.position.state);
        fsm.back(element, depth, self.scope_depth)
    }

    fn get_parent(&self) -> usize {
        self.parent
    }

    fn set_parent(&mut self, value: usize) {
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
    use super::{Fsm, FsmState};
    use crate::Query;
    use crate::css::parser::tree::Save;
    use crate::xhtml::element::element::XHtmlElement;

    #[test]
    fn test_fsm_next_descendant() {
        let selection_tree = Query::all("div a", Save::none()).build();

        let mut state = FsmState::new();
        let mut next: bool = false;

        next = state.next(
            &selection_tree,
            0,
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
            },
        );

        assert!(next);

        // move task
        //state.step_foward(&selection_tree, 0);
        let position = state.position.next_state(&selection_tree);
        state.position.state = position.unwrap();

        next = state.next(
            &selection_tree,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![],
            },
        );

        assert!(next);
    }
}
