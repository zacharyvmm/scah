use super::fsm::Selection;
use super::parser::pattern::Pattern;
use super::tree::Tree;

use crate::XHtmlElement;
use crate::css::parser::pattern::NextPosition;

/*
 * Fsm Session should have the following abilities:
 * 1) try step foward at fsm cursor
 * 2) retry fsm pattern segments beginning (previous to the current cursor) -> branch the tree and make new fsm
 * 3) when fsm completed it has to backtrack when going back to add the InnerHtml and TextContent
 * 4) Send the agrogated information back to the caller
 */

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

    pub fn next(&mut self, element: &XHtmlElement<'html>, depth: usize) {
        let mut new_fsms: Vec<(usize, Selection)> = Vec::new();
        for (previous_node_idx, fsm) in self.fsms.iter_mut() {
            let next = fsm.next(&self.pattern, element, depth);
            // TODO: if the pattern section (ex: their are 3 branches) changed the must reassign this fsm (ex: the first branch) to the first branch and create other branches (#2 and #3 of the branches).

            match next {
                NextPosition::Link(pat, pos) => {
                    (fsm.pattern, fsm.position) = (pat, pos);
                }
                NextPosition::Fork(mut selections) => {
                    // first is a reassign of the current fsm
                    let first = selections
                        .pop()
                        .expect("There has to be an element in the vector");
                    (fsm.pattern, fsm.position) = (first.0, first.1);

                    for (pattern, position) in selections {
                        let mut new_fsm = Selection::new();
                        (new_fsm.pattern, new_fsm.position) = (pattern, position);
                        new_fsms.push((*previous_node_idx, new_fsm));
                    }

                    // the other are created
                }
                NextPosition::NoMovement => continue,
            }

            // TODO: `element.clone()` inefficient for if their's only a single selection saving the element (99.99% of the time)
            *previous_node_idx = self.tree.push(*previous_node_idx, element.clone());
        }

        self.fsms.append(&mut new_fsms);
    }

    pub fn back(&self, depth: usize) {}

    fn reevaluate_descendants(&mut self, element: &XHtmlElement<'html>, depth: usize) {
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
    use crate::css::parser::pattern::{Mode, PatternSection};
    use crate::utils::reader::Reader;

    use super::*;

    #[test]
    fn test_fsm_with_multi_pattern_sections() {
        let first = PatternSection::new(&mut Reader::new("main.indent > .bold"), Mode::All);
        let second = PatternSection::new(&mut Reader::new(" p"), Mode::All);
        let second_alternate = PatternSection::new(&mut Reader::new("> span > a"), Mode::All);
        let mut builder = Pattern::new(Vec::from([first]));
        builder.append(Vec::from([second, second_alternate]));
        let mut session = FsmSession::new(&builder);

        session.next(
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        session.next(
            &XHtmlElement {
                name: "main",
                id: Some("name"),
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        session.next(
            &XHtmlElement {
                name: "div",
                id: Some("name"),
                class: Some(".bold"),
                attributes: Vec::new(),
            },
            1,
        );

        session.next(
            &XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some(".class"),
                attributes: Vec::new(),
            },
            2,
        );

        session.next(
            &XHtmlElement {
                name: "p",
                id: Some("name"),
                class: Some(".bold"),
                attributes: Vec::new(),
            },
            3,
        );

        session.next(
            &XHtmlElement {
                name: "a",
                id: Some("name"),
                class: Some(".class"),
                attributes: Vec::new(),
            },
            3,
        );
    }
}
