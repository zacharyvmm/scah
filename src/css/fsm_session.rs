use super::fsm::Selection;
use super::parser::pattern_section::PatternSection;
use super::parser::query_tokenizer::{Combinator, PatternStep};
use super::tree::Tree;

use crate::XHtmlElement;

/*
 * Fsm Session should have the following abilities:
 * 1) try step foward at fsm cursor
 * 2) retry fsm pattern segments beginning (previous to the current cursor) -> branch the tree and make new fsm
 * 3) when fsm completed it has to backtrack when going back to add the InnerHtml and TextContent
 * 4) Send the agrogated information back to the caller
 */

struct Pattern<'query> {
    descendants: Vec<Vec<usize>>,
    pub list: &'query Vec<PatternSection<'query>>,
}

enum NextPosition<U, T> {
    Link(U, T),
    Fork(Vec<(U, T)>),
}

impl<'query> Pattern<'query> {
    fn generate_descendants(&mut self) {
        for section_index in 0..self.list.len() {
            let mut section_descendants = Vec::new();
            let pattern_steps = &self.list[section_index].pattern;
            for index in 0..pattern_steps.len() {
                if pattern_steps[index] == PatternStep::Combinator(Combinator::Descendant) {
                    section_descendants.push(index);
                }
            }
            self.descendants.push(section_descendants);
        }
    }

    pub fn new(list: &'query Vec<PatternSection<'query>>) -> Self {
        let mut pattern = Self {
            list: list,
            descendants: Vec::with_capacity(list.len()),
        };
        pattern.generate_descendants();

        return pattern;
    }

    pub fn next(&self, section_index: usize, step_index: usize) -> NextPosition<usize, usize> {
        assert!(section_index < self.list.len());
        assert!(step_index < self.list[section_index].len());

        if step_index + 1 < self.list[section_index].len() {
            return NextPosition::Link(section_index, step_index + 1);
        } else {
            let ref section = self.list[section_index];
            return NextPosition::Fork(
                section
                    .children
                    .iter()
                    .map(|child_index| (*child_index, 0))
                    .collect(),
            );
        }
    }

    pub fn back(&self, section_index: usize, step_index: usize) -> (usize, usize) {
        assert!(section_index < self.list.len());
        assert!(step_index < self.list[section_index].len());

        if step_index > 0 {
            return (section_index, step_index - 1);
        } else {
            let ref section = self.list[section_index];
            if let Some(parent) = section.parent {
                return (parent, section.len() - 1);
            } else {
                return (0, 0);
            }
        }
    }

    pub fn last_descendant(
        &self,
        fsm_section_index: usize,
        fsm_step_index: usize,
    ) -> (usize, usize) {
        // BUG The pattern sections are not linkedlist it's a tree so in reality this need to check the parent
        for section_index in (0..fsm_section_index + 1).rev() {
            for step_index in (&self.descendants[section_index]).into_iter().rev() {
                if *step_index < fsm_step_index {
                    return (section_index, *step_index);
                }
            }
        }
        return (0, 0);
    }
}

/*
 * Way's to add a fsm:
 * 1) at init you create a fsm
 * 2) if the input element matches the lowest scope (lowest decendant or root)
 * 3) at a forking in fsm tree; the current fsm is reassigned (takes the first branch) and creates new fsm's for the other branches
*/
pub struct FsmSession<'query, 'html> {
    pattern: &'query Pattern<'query>,
    tree: Tree<'html>,

    fsms: Vec<(usize, Selection)>,
}

impl<'query, 'html> FsmSession<'query, 'html> {
    fn new(pattern: &'query Pattern<'query>) -> Self {
        Self {
            pattern: pattern,
            tree: Tree::new(),

            fsms: Vec::from([(0, Selection::new())]),
        }
    }

    pub fn step_foward(&mut self, depth: usize, element: &XHtmlElement<'html>) {
        for (previous_node_idx, fsm) in self.fsms.iter_mut() {
            let next = fsm.next(self.pattern.list, element, depth);
            // TODO: if the pattern section (ex: their are 3 branches) changed the must reassign this fsm (ex: the first branch) to the first branch and create other branches (#2 and #3 of the branches).

            if next {
                // TODO: `element.clone()` inefficient for if their's only a single selection saving the element (99.99% of the time)
                *previous_node_idx = self.tree.push(*previous_node_idx, element.clone());
            }
        }
    }

    pub fn step_back(&self, depth: usize) {}

    fn reevaluate_descendants(&mut self, depth: usize, element: &XHtmlElement<'html>) {
        // The first node is always reevaluated (BECAUSE the start of a query is basicly a Descendant selection) unless
        // When you are already in a descendant scope the descendant element is reevaluated
        // Once the fsm's current combinator is not a descendant selection a fsm with the lowest descendant selection is created
        // for example: `html main section > div > a`, if you are now at `div` you must create a `section` selector for the inner scope of div (just in case a `section` is within the `div` or `a`)
        // Inner Scope fsm, this is a fsm with that litterary starts within the scope and dies at the end of the scope, thus this handles the recursion problem

        // The fsm reevalutes the first element in the pattern sections and if one exists a new fsm is created
        // for example on ever cycle the first node is reevaluated

        //  for each begining_element in sections
        //      if begining_element == element and descendant_selector
        //          check every fsm that's been their and add this element
    }

    fn bubble_up_content_fill_in() {}
}

#[cfg(test)]
mod tests {
    use crate::{
        css::parser::{fsm_builder::FsmBuilder, pattern_section::Mode},
        utils::reader::Reader,
    };

    use super::*;

    #[test]
    fn test_mock_fsm_assign_to_tree() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let mut then_a_reader = Reader::new("> a");
        let mut then_p_reader = Reader::new(" p");

        let section = PatternSection::new(&mut reader, Mode::All);
        let mut selection = FsmBuilder::new(Vec::from([section]));

        selection.append(Vec::from([
            PatternSection::new(&mut then_a_reader, Mode::All),
            PatternSection::new(&mut then_p_reader, Mode::All),
        ]));

        let pattern = Pattern::new(&selection.list);

        let mut session = FsmSession::new(&pattern);

        // 1: fsm first position
        // 1: fsm second position
        // 2: fsm first position
        // 1.2: fsmn second alternate position
    }
}
