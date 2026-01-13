use std::fmt::Debug;

use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{NextPosition, NextPositions, Position, Query};
use crate::store::ROOT;
use smallvec::SmallVec;

#[derive(PartialEq, Debug)]
pub struct FsmState<E>
where
    E: Debug,
{
    pub(super) parent: E,
    pub(super) position: Position,
    pub(super) depths: SmallVec<[super::DepthSize; 10]>,
    pub(super) end: bool, // This is a flag to say is a save point and this might be the end
}

pub trait Fsm<'query, 'html, E> {
    fn next(
        &self,
        tree: &Query<'query>,
        depth: super::DepthSize,
        element: &XHtmlElement<'html>,
    ) -> bool;
    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &'html str) -> bool;
    fn try_back_parent(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str)
    -> bool;

    fn move_foward(
        &mut self,
        tree: &Query<'query>,
        depth: super::DepthSize,
    ) -> Option<NextPositions>;
    fn move_backward(&mut self, tree: &Query<'query>);

    fn is_descendant(&self, tree: &Query<'query>) -> bool;
    fn is_save_point(&self, tree: &Query<'query>) -> bool;
    fn is_last_save_point(&self, tree: &Query<'query>) -> bool;

    fn get_parent(&self) -> E;
    fn set_parent(&mut self, value: E);

    fn get_section(&self) -> usize;
    fn set_end_false(&mut self);
}

impl<'query, E> FsmState<E>
where
    E: Default + Copy + Debug,
{
    pub fn new() -> Self {
        Self {
            parent: E::default(),
            position: Position { section: 0, fsm: 0 },
            depths: SmallVec::new(),
            end: false,
        }
    }

    pub fn move_backward_twice(&mut self, tree: &Query<'query>) {
        // Only need one pop, since the current fsm depth was not added to the list
        self.move_backward(tree);
        self.position = tree.back(&self.position);
    }
}

impl<'query, 'html, E> Fsm<'query, 'html, E> for FsmState<E>
where
    E: Copy + Debug,
{
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get(&self.position);
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.next(element, depth, last_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get(&self.position);
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    // Try going backwards from a parent of a leaf
    fn try_back_parent(
        &self,
        tree: &Query<'query>,
        depth: super::DepthSize,
        element: &str,
    ) -> bool {
        debug_assert!(self.end);

        if (self.position == Position { section: 0, fsm: 0 }) {
            return false;
        }

        let parent_position = tree.back(&self.position);
        assert_ne!(self.position, parent_position);
        let fsm = tree.get(&parent_position);

        // BUG: I'm not sure if I should take the last or the one before
        // What happens at length 0 or 1?
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    fn move_foward(
        &mut self,
        tree: &Query<'query>,
        depth: super::DepthSize,
    ) -> Option<NextPositions> {
        let positions = tree.next(&self.position);
        //if tree.is_last_save_point(1)
        self.depths.push(depth);

        match positions {
            NextPosition::Link(pos) => {
                self.position = pos;
                None
            }
            NextPosition::Fork(mut pos_list) => {
                assert_ne!(pos_list.len(), 0, "Fork with no positions");

                self.position = pos_list.pop()?;

                Some(pos_list)
            }

            NextPosition::EndOfBranch => None,
        }
    }

    fn move_backward(&mut self, tree: &Query<'query>) {
        // BUG: Currently this works for opening a closing element's, but if in a ALL selection
        // The FSM position and make it break
        assert!(self.depths.len() > 0);
        self.depths.pop();

        self.position = tree.back(&self.position);
    }

    fn is_descendant(&self, tree: &Query<'query>) -> bool {
        tree.get(&self.position).transition == Combinator::Descendant
    }

    fn is_save_point(&self, tree: &Query<'query>) -> bool {
        tree.is_save_point(&self.position)
    }

    fn is_last_save_point(&self, tree: &Query<'query>) -> bool {
        tree.is_last_save_point(&self.position)
    }

    fn get_parent(&self) -> E {
        self.parent
    }

    fn set_parent(&mut self, value: E) {
        self.parent = value;
    }

    fn get_section(&self) -> usize {
        self.position.section
    }

    fn set_end_false(&mut self) {
        self.end = false;
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ScopedFsm<E> {
    pub scope_depth: super::DepthSize,
    pub parent: E,
    pub position: Position,
}

impl<'query, E> ScopedFsm<E> {
    pub fn new(scope_depth: super::DepthSize, parent: E, position: Position) -> Self {
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

impl<'query, 'html, E> Fsm<'query, 'html, E> for ScopedFsm<E>
where
    E: Copy,
{
    fn next(&self, tree: &Query<'query>, depth: super::DepthSize, element: &XHtmlElement) -> bool {
        let fsm = tree.get(&self.position);
        fsm.next(element, depth, self.scope_depth)
    }

    fn back(&self, tree: &Query<'query>, depth: super::DepthSize, element: &str) -> bool {
        let fsm = tree.get(&self.position);
        fsm.back(element, depth, self.scope_depth)
    }

    // Try going backwards from a parent of a leaf
    fn try_back_parent(
        &self,
        tree: &Query<'query>,
        depth: super::DepthSize,
        element: &str,
    ) -> bool {
        let parent_position = tree.back(&self.position);
        let fsm = tree.get(&parent_position);

        fsm.back(element, depth, self.scope_depth)
    }

    fn move_foward(
        &mut self,
        tree: &Query<'query>,
        depth: super::DepthSize,
    ) -> Option<NextPositions> {
        let positions = tree.next(&self.position);
        //if tree.is_last_save_point(1)
        self.scope_depth = depth;

        match positions {
            NextPosition::Link(pos) => {
                self.position = pos;
                None
            }
            NextPosition::Fork(mut pos_list) => {
                assert_ne!(pos_list.len(), 0, "Fork with no positions");

                self.position = pos_list.pop()?;

                Some(pos_list)
            }

            NextPosition::EndOfBranch => None,
        }
    }

    fn is_descendant(&self, tree: &Query<'query>) -> bool {
        tree.get(&self.position).transition == Combinator::Descendant
    }

    fn is_save_point(&self, tree: &Query<'query>) -> bool {
        tree.is_save_point(&self.position)
    }

    fn is_last_save_point(&self, tree: &Query<'query>) -> bool {
        tree.is_last_save_point(&self.position)
    }

    fn get_parent(&self) -> E {
        self.parent
    }

    fn set_parent(&mut self, value: E) {
        self.parent = value;
    }

    fn get_section(&self) -> usize {
        self.position.section
    }

    fn move_backward(&mut self, _tree: &Query<'query>) {}
    fn set_end_false(&mut self) {}
}
mod tests {
    use super::{Fsm, FsmState};
    use crate::Query;
    use crate::css::parser::tree::{Save, SelectionPart};
    use crate::store::Element;
    use crate::utils::Reader;
    use crate::xhtml::element::element::XHtmlElement;

    #[test]
    fn test_fsm_next_descendant() {
        let selection_tree = Query::all("div a", Save::none()).build();

        let mut state = FsmState::<usize>::new();
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
        state.move_foward(&selection_tree, 0);

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
