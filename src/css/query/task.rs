// A Selection Runner
use crate::XHtmlElement;
use crate::css::parser::tree::{NextPosition, Position, SelectionTree};

#[derive(PartialEq, Debug, Clone)]
pub struct FsmState {
    pub(super) parent_tree_position: usize,
    pub(super) position: Position,
    pub(super) depths: Vec<usize>,
}

impl<'query> FsmState {
    pub fn new() -> Self {
        Self {
            parent_tree_position: 0,
            position: Position { section: 0, fsm: 0 },
            depths: vec![],
        }
    }

    pub fn next(&self, tree: &SelectionTree<'query>, depth: usize, element: &XHtmlElement) -> bool {
        let fsm = tree.get(&self.position);
        fsm.next(element, depth, *self.depths.last().unwrap_or(&0))
    }

    pub fn back(&self, tree: &SelectionTree<'query>, depth: usize, element: &str) -> bool {
        let fsm = tree.get(&self.position);
        fsm.back(element, depth, *self.depths.last().unwrap_or(&0))
    }

    pub fn move_foward(
        &mut self,
        tree: &SelectionTree<'query>,
        depth: usize,
    ) -> Option<Vec<Position>> {
        let positions = tree.next(&self.position);
        self.depths.push(depth);

        match positions {
            NextPosition::Link(pos) => {
                self.position = pos;
                None
            }
            NextPosition::Fork(mut pos_list) => {
                if pos_list.len() == 0 {
                    panic!("Fork with no positions");
                }

                self.position = pos_list.pop()?;

                Some(pos_list)
            }

            NextPosition::EndOfBranch => None,
        }
    }

    pub fn move_backward(&mut self, tree: &SelectionTree<'query>) {
        assert!(self.depths.len() > 0);
        self.depths.pop();

        self.position = tree.back(&self.position);
    }
}

pub struct Task {
    retry_from: Option<FsmState>,
    state: FsmState,
}

impl<'query> Task {
    pub fn new(task: FsmState) -> Self {
        Self {
            retry_from: None,
            state: task,
        }
    }

    pub fn set_retry(&mut self, retry_task: FsmState) {
        self.retry_from = Some(retry_task);
    }

    pub fn retry(
        &mut self,
        tree: &SelectionTree<'query>,
        depth: usize,
        element: &XHtmlElement,
    ) -> Option<FsmState> {
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

pub struct ScopedTask {
    scope_depth: usize,
    task: Task,
}

impl ScopedTask {
    fn new(depth: usize, origin_state: &FsmState) -> Self {
        Self {
            scope_depth: depth,
            task: Task {
                retry_from: None,
                state: FsmState {
                    parent_tree_position: origin_state.parent_tree_position,
                    position: origin_state.position.clone(),
                    depths: vec![],
                },
            },
        }
    }

    fn in_scope(&self, current_depth: usize) -> bool {
        self.scope_depth > current_depth
    }
}

mod tests {
    use crate::css::parser::tree::{Save, SelectionKind, SelectionPart};
    use crate::utils::Reader;

    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let mut reader = Reader::new("div a");

        let section = SelectionPart::new(
            &mut reader,
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = SelectionTree::new(Vec::from([section]));

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
