// A Selection Runner
use crate::XHtmlElement;
use crate::css::parser::tree::{NextPosition, Position, SelectionTree};

#[derive(PartialEq, Debug, Clone)]
pub struct Task {
    pub(super) parent_tree_position: usize,
    pub(super) position: Position,
    pub(super) depths: Vec<usize>,
}

impl<'query> Task {
    pub fn new() -> Self {
        Self {
            parent_tree_position: 0,
            position: Position { section: 0, fsm: 0 },
            depths: vec![],
        }
    }

    pub fn next(
        &mut self,
        tree: &SelectionTree<'query>,
        depth: usize,
        element: &XHtmlElement,
    ) -> bool {
        let fsm = tree.get(&self.position);
        fsm.next(element, depth, *self.depths.last().unwrap_or(&0))
    }

    pub fn back(&mut self, tree: &SelectionTree<'query>, depth: usize, element: &str) -> bool {
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
