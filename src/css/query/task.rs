// A Selection Runner
use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{NextPosition, Position, Selection};
use std::ptr;

#[derive(PartialEq, Debug)]
pub struct FsmState<E> {
    pub(super) parent: *mut E,
    pub(super) position: Position,
    pub(super) depths: Vec<usize>,
    pub(super) end: bool, // This is a flag to say is a save point and this might be the end
}

impl<E> Clone for FsmState<E> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent,
            position: self.position.clone(),
            depths: vec![],
            end: false,
        }
    }
}

impl<'query, E> FsmState<E> {
    pub fn new() -> Self {
        Self {
            parent: ptr::null_mut(),
            position: Position { section: 0, fsm: 0 },
            depths: vec![],
            end: false,
        }
    }

    pub fn next(&self, tree: &Selection<'query>, depth: usize, element: &XHtmlElement) -> bool {
        let fsm = tree.get(&self.position);
        fsm.next(element, depth, *self.depths.last().unwrap_or(&0))
    }

    pub fn back(&self, tree: &Selection<'query>, depth: usize, element: &str) -> bool {
        let fsm = tree.get(&self.position);
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    // Try going backwards from a parent of a leaf
    pub fn try_back_parent(&self, tree: &Selection<'query>, depth: usize, element: &str) -> bool {
        debug_assert!(self.end);
        let parent_position = tree.back(&self.position);
        let fsm = tree.get(&parent_position);

        // BUG: I'm not sure if I should take the last or the one before
        // What happens at length 0 or 1?
        let last_depth = *self.depths.last().unwrap_or(&0);
        fsm.back(element, depth, last_depth)
    }

    pub fn move_foward(&mut self, tree: &Selection<'query>, depth: usize) -> Option<Vec<Position>> {
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

    pub fn move_backward(&mut self, tree: &Selection<'query>) {
        // BUG: Currently this works for opening a closing element's, but if in a ALL selection
        // The FSM position and make it break
        assert!(self.depths.len() > 0);
        self.depths.pop();

        self.position = tree.back(&self.position);
    }

    pub fn is_descendant(&self, tree: &Selection<'query>) -> bool {
        tree.get(&self.position).transition == Combinator::Descendant
    }

    pub fn is_save_point(&self, tree: &Selection<'query>) -> bool {
        tree.is_save_point(&self.position)
    }

    pub fn is_last_save_point(&self, tree: &Selection<'query>) -> bool {
        tree.is_last_save_point(&self.position)
    }
}

#[derive(PartialEq, Debug)]
pub struct ScopedFsm<E> {
    pub scope_depth: usize,
    pub fsm: FsmState<E>,
}

impl<E> Clone for ScopedFsm<E> {
    fn clone(&self) -> Self {
        Self {
            scope_depth: self.scope_depth,
            fsm: self.fsm.clone(),
        }
    }
}

impl<E> ScopedFsm<E> {
    pub fn new(depth: usize, parent: *mut E, position: Position) -> Self {
        Self {
            scope_depth: depth,
            fsm: FsmState {
                parent: parent,
                position: position,
                depths: vec![],
                end: false,
            },
        }
    }

    pub fn in_scope(&self, current_depth: usize) -> bool {
        self.scope_depth > current_depth
    }
}

mod tests {
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
        let selection_tree = Selection::new(section);

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
