use super::pattern::Pattern;
use super::state::{Fsm, SelectionKind};
use super::tree::Tree;
use crate::XHtmlElement;

struct Selection<'query, 'html> {
    fsms: &'query Vec<Fsm<'query>>,
    patterns: Vec<Pattern>,
    retry_points: Vec<Pattern>,
    tree: Tree<'html>,
}

impl<'query, 'html> Selection<'query, 'html> {
    fn new(fsms: &'query Vec<Fsm<'query>>) -> Self {
        Self {
            fsms,
            patterns: Vec::new(),
            retry_points: Vec::new(),
            tree: Tree::new(),
        }
    }

    fn next(&mut self, fsms: &Vec<Fsm>, depth: usize, element: &XHtmlElement<'html>) {
        for i in 0..self.patterns.len() {
            let ref mut pattern = self.patterns[i];

            if pattern.next(fsms, depth, element) {
                let kind = &self.fsms[pattern.position].state_kind;

                if *kind == SelectionKind::None {
                    pattern.move_foward(depth);
                    return;
                }

                pattern.parent_save_position = self
                    .tree
                    .push(pattern.parent_save_position, element.clone());

                pattern.move_foward(depth);
                if matches!(*kind, SelectionKind::All(..)) {
                    return;
                }

                // Step 3: Remove the pattern (if no textContent/innerHtml is needed)
                // Go back to the last ALL selection in the fsms list
                // If their are none then the whole selection is done
            }
        }
    }

    fn back(&mut self, fsms: &Vec<Fsm>, depth: usize, element: &'html str) {
        for i in 0..self.patterns.len() {
            let ref mut pattern = self.patterns[i];

            if pattern.back(fsms, depth, element) {
                if self.fsms[pattern.position].state_kind != SelectionKind::None {
                    // TODO: Add real Content
                    self.tree.set_content(pattern.parent_save_position, None, None);
                }

                pattern.move_backward();
            }
        }
    }
}
