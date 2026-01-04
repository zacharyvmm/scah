use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{NextPosition, NextPositions, Position, Query};
use smallvec::SmallVec;
use std::ptr;

#[derive(PartialEq)]
pub struct FsmState<E> {
    pub(super) parent: *mut E,
    pub(super) position: Position,
    pub(super) depths: SmallVec<[super::DepthSize; 10]>,
    pub(super) end: bool, // This is a flag to say is a save point and this might be the end
}

impl<E> std::fmt::Debug for FsmState<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsmState")
            .field("parent", &self.parent)
            .field("position", &self.position)
            .field("depths", &self.depths)
            .field("end", &self.end)
            .finish()
    }
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

    fn get_parent(&self) -> *mut E;
    fn set_parent(&mut self, value: *mut E);
    fn get_section(&self) -> usize;
    fn set_end_false(&mut self);
}

impl<'query, E> FsmState<E> {
    pub fn new() -> Self {
        Self {
            parent: ptr::null_mut(),
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

impl<'query, 'html, E> Fsm<'query, 'html, E> for FsmState<E> {
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

    fn get_parent(&self) -> *mut E {
        self.parent
    }

    fn set_parent(&mut self, value: *mut E) {
        self.parent = value;
    }

    fn get_section(&self) -> usize {
        self.position.section
    }

    fn set_end_false(&mut self) {
        self.end = false;
    }
}

#[derive(PartialEq)]
pub struct ScopedFsm<E> {
    pub scope_depth: super::DepthSize,
    pub parent: *mut E,
    pub position: Position,
}

impl<'query, E> Clone for ScopedFsm<E> {
    fn clone(&self) -> Self {
        Self {
            scope_depth: self.scope_depth,
            parent: self.parent,
            position: self.position,
        }
    }
}

impl<E> std::fmt::Debug for ScopedFsm<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopedFsm")
            .field("scope_depth", &self.scope_depth)
            // This will print the raw memory address (e.g., 0x7ff...)
            // It is safe because we are not dereferencing it.
            .field("parent", &self.parent)
            .field("position", &self.position)
            .finish()
    }
}

impl<'query, E> ScopedFsm<E> {
    pub fn new(depth: super::DepthSize, parent: *mut E, position: Position) -> Self {
        Self {
            scope_depth: depth,
            parent: parent,
            position: position,
        }
    }

    pub fn in_scope(&self, current_depth: super::DepthSize) -> bool {
        self.scope_depth > current_depth
    }
}

impl<'query, 'html, E> Fsm<'query, 'html, E> for ScopedFsm<E> {
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

    fn get_parent(&self) -> *mut E {
        self.parent
    }

    fn set_parent(&mut self, value: *mut E) {
        self.parent = value;
    }

    fn get_section(&self) -> usize {
        self.position.section
    }

    fn move_backward(&mut self, tree: &Query<'query>) {}
    fn set_end_false(&mut self) {}
}
mod tests {
    use crate::QueryBuilder;
    use crate::css::parser::tree::{Save, SelectionKind, SelectionPart};
    use crate::store::Element;
    use crate::utils::Reader;

    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let section = SelectionPart::new(
            "div a",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = QueryBuilder::new(section).build();

        let mut state = FsmState::<Element>::new();
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
