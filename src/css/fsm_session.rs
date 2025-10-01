use super::fsm::Selection;
use super::parser::pattern_section::PatternSection;
use crate::css::selection_map::Select;
use crate::{css::parser::element, utils::tree::Tree};

use crate::XHtmlElement;

/*
 * Fsm Session should have the following abilities:
 * 1) try step foward at fsm cursor
 * 2) retry fsm pattern segments beginning (previous to the current cursor) -> branch the tree and make new fsm
 * 3) when fsm completed it has to backtrack when going back to add the InnerHtml and TextContent
 * 4) Send the agrogated information back to the caller
 */

pub struct FsmSession<'query, 'html> {
    list: Vec<PatternSection<'query>>,
    tree: Tree<'html>,

    fsms: Vec<(usize, Selection)>,
}

impl<'query, 'html> FsmSession<'query, 'html> {
    fn new(pattern: Vec<PatternSection<'query>>) -> Self {
        Self {
            list: pattern,
            tree: Tree::new(),

            fsms: Vec::new(),
        }
    }

    fn create_fsm() -> Selection {
        Selection::new()
    }

    pub fn step_foward(&mut self, depth: usize, element: &XHtmlElement<'html>) {
        for (previous_node_idx, fsm) in self.fsms.iter_mut() {
            let next = fsm.next(&self.list, element, depth);

            if next {
                // TODO: `element.clone()` inefficient for if their's only a single selection saving the element (99.99% of the time)
                *previous_node_idx = self.tree.push(*previous_node_idx, element.clone());
            }
        }
    }

    pub fn step_back(&self, depth: usize) {}

    fn retry_previous_patterns() {}

    fn bubble_up_content_fill_in() {}
}
