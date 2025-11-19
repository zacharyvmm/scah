// A Selection Runner
use crate::XHtmlElement;
use crate::css::parser::tree::{NextPosition, Position, Selection};
use std::ptr;

#[derive(PartialEq, Debug)]
pub struct FsmState<E> {
    pub(super) parent: *mut E,
    pub(super) position: Position,
    pub(super) depths: Vec<usize>,
}

impl<E> Clone for FsmState<E> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent,
            position: self.position.clone(),
            depths: self.depths.clone(),
        }
    }
}

impl<'query, E> FsmState<E> {
    pub fn new() -> Self {
        Self {
            parent: ptr::null_mut(),
            position: Position { section: 0, fsm: 0 },
            depths: vec![],
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
}

#[derive(PartialEq, Debug)]
pub struct Task<E> {
    pub retry_from: Option<FsmState<E>>,
    pub state: FsmState<E>,
}

impl<E> Clone for Task<E> {
    fn clone(&self) -> Self {
        Self {
            retry_from: self.retry_from.clone(),
            state: self.state.clone(),
        }
    }
}

impl<'query, E> Task<E> {
    pub fn new(task: FsmState<E>) -> Self {
        Self {
            retry_from: None,
            state: task,
        }
    }

    pub fn set_retry(&mut self, retry_task: FsmState<E>) {
        self.retry_from = Some(retry_task);
    }

    pub fn retry(
        &mut self,
        tree: &Selection<'query>,
        depth: usize,
        element: &XHtmlElement,
    ) -> Option<FsmState<E>> {
        //let old_task: Option<Task> = self.retry_from.take();
        let mut retry_task = false;
        if let Some(task) = &self.retry_from {
            retry_task = task.next(tree, depth, element);
        }
        if retry_task {
            return self.retry_from.take();
        }

        return None;
    }
}

#[derive(PartialEq, Debug)]
pub struct ScopedTask<E> {
    pub scope_depth: usize,
    pub task: Task<E>,
}

impl<E> Clone for ScopedTask<E> {
    fn clone(&self) -> Self {
        Self {
            scope_depth: self.scope_depth,
            task: self.task.clone(),
        }
    }
}

impl<E> ScopedTask<E> {
    pub fn new(depth: usize, parent: *mut E, position: Position) -> Self {
        Self {
            scope_depth: depth,
            task: Task {
                retry_from: None,
                state: FsmState {
                    parent: parent,
                    position: position,
                    depths: vec![],
                },
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
