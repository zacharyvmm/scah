use super::parser::pattern::PatternSection;
use super::parser::query_tokenizer::Combinator;
use super::parser::query_tokenizer::PatternStep;
use crate::xhtml::element::element::XHtmlElement;

#[derive(Debug, Clone)]
pub struct Selection {
    element_depth_stack: Vec<usize>,
    pattern: usize,
    position: usize,
    // start_position: usize,
    /*
     * When `start_position - position <= 0` then the fsm is deleted
     * this allows the fsm to be confined to a specific scope
     */
}

impl Selection {
    pub fn new() -> Self {
        Self {
            element_depth_stack: Vec::new(),
            pattern: 0,
            position: 0,
        }
    }

    fn step_back<'query>(&mut self, query: &Vec<PatternSection<'query>>) {
        if self.position > 0 {
            self.position -= 1;
            return;
        }

        assert_ne!(self.pattern, 0);
        self.pattern -= 1;
        let last_idx_of_previous_pattern_section = &query[self.pattern].pattern.len() - 1;
        if query[self.pattern].pattern[last_idx_of_previous_pattern_section] == PatternStep::Save {
            self.position = last_idx_of_previous_pattern_section - 1;
        } else {
            self.position = last_idx_of_previous_pattern_section;
        }
    }

    fn step_foward<'query>(&mut self, query: &Vec<PatternSection<'query>>) {
        if self.position + 1 < query[self.pattern].pattern.len() {
            self.position += 1;
            return;
        }

        assert_ne!(self.pattern, query[self.pattern].pattern.len() - 1);
        // BUG: This assumes that the tree is a linked list
        self.pattern += 1;
        self.position = 0;
    }

    fn next_until_element<'query>(&mut self, query: &Vec<PatternSection<'query>>) {
        let current = &query[self.pattern].pattern[self.position];
        if let PatternStep::Combinator(_) = current {
            self.step_foward(query);
        }
        assert!(matches!(
            query[self.pattern].pattern[self.position],
            PatternStep::Element(..) | PatternStep::Save
        ));
    }

    fn previous_child_combinator_invalid<'query>(
        &mut self,
        pattern: &Vec<PatternStep>,
        depth: usize,
    ) -> bool {
        if self.position == 0 {
            return false;
        }

        let previous = &pattern[self.position - 1];

        // assert: not Element
        assert!(!matches!(previous, &PatternStep::Element(..)));

        if let PatternStep::Combinator(Combinator::Child) = previous {
            let element_depth = self
                .element_depth_stack
                .last()
                .expect("Their must be a item in the stack");
            return element_depth + 1 != depth;
        } else {
            false
        }
    }

    // NOTE: In the case of failure IF last element is NextSibling, then go back to the Element that the NextSibling is referencing
    fn next_sibling_failed_move_back<'query>(&mut self, query: &Vec<PatternSection<'query>>) {
        if self.pattern == 0 && self.position == 0 {
            return;
        }

        let previous = &query[self.pattern].pattern[self.position - 1];

        if let PatternStep::Combinator(Combinator::NextSibling) = previous {
            self.step_back(query);
            assert!(matches!(
                &query[self.pattern].pattern[self.position],
                &PatternStep::Element(..)
            ));
            self.step_back(query); // Last Element or Save
            if query[self.pattern].pattern[self.position] == PatternStep::Save {
                self.step_back(query);
            }

            assert!(matches!(
                &query[self.pattern].pattern[self.position],
                &PatternStep::Element(..)
            ));
        }
    }

    pub fn is_reset(&self) -> bool {
        self.position == 0 && self.pattern == 0 && self.element_depth_stack.len() == 0
    }

    pub fn reset(&mut self) {
        self.pattern - 0;
        self.position = 0;

        self.element_depth_stack.clear();
    }

    pub fn next<'query, 'html>(
        &mut self,
        query: &Vec<PatternSection<'query>>,
        xhtml_element: &XHtmlElement<'html>,
        stack_depth: usize,
    ) -> bool {
        if self.previous_child_combinator_invalid(&query[self.pattern].pattern, stack_depth) {
            return false;
        }

        // TODO: Refactor this to a match to handle `Has` and `Not`

        if let PatternStep::Element(ref element) = query[self.pattern].pattern[self.position] {
            // NOTE: Compare xhtml element to selector element
            if element == xhtml_element {
                self.step_foward(query);

                self.element_depth_stack.push(stack_depth);

                self.next_until_element(query);
                return true;
            }

            self.next_sibling_failed_move_back(query);

            return false;
        }
        return false;
    }

    pub fn back<'query>(&mut self, query: &Vec<PatternSection<'query>>, depth: usize) {
        if self.position == 0 {
            return;
        }

        if query[self.pattern].pattern[self.position] == PatternStep::Save {
            if self.position == 0 {
                self.pattern -= 1;
            } else {
                self.position -= 1;
            }

            assert!(
                matches!(
                    query[self.pattern].pattern[self.position],
                    PatternStep::Element(..)
                ),
                "The item in the query list before a child combinator should always be an `Element`."
            );
            self.element_depth_stack.pop();
            return;
        }

        let mut element_position = self.position;
        //assert!(!matches!(self.query[element_position], PatternStep::Element(..)));
        if !matches!(
            query[self.pattern].pattern[element_position],
            PatternStep::Element(..)
        ) {
            element_position = self.position - 1;
        }

        let element_depth = self
            .element_depth_stack
            .last()
            .expect("Must have an element in the stack");

        if *element_depth == depth {
            assert!(element_position >= 2);
            self.position = element_position - 2;
            self.element_depth_stack.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::pattern::{Mode, Pattern, PatternSection};
    use super::*;
    use crate::utils::reader::Reader;

    #[test]
    fn test_selection_fsm_element_name_selection() {
        let mut reader = Reader::new("element > p");
        let pattern = PatternSection::new(&mut reader, Mode::All);
        let builder = Pattern::new(Vec::from([pattern]));
        let mut selection = Selection::new();

        let mut fsm_moved: bool = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "element",
                id: None,
                class: None,
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "p",
                id: None,
                class: None,
                attributes: Vec::new(),
            },
            0,
        );

        assert_eq!(fsm_moved, false);

        fsm_moved = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "p",
                id: None,
                class: None,
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(
            builder.list[selection.pattern].pattern[selection.position],
            PatternStep::Save
        );
    }

    #[test]
    fn test_selection_fsm_complex_element_selection() {
        let mut reader = Reader::new("p.indent > .bold");
        let pattern = PatternSection::new(&mut reader, Mode::All);
        let builder = Pattern::new(Vec::from([pattern]));
        let mut selection = Selection::new();

        let mut fsm_moved: bool = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some("bold"),
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(
            builder.list[selection.pattern].pattern[selection.position],
            PatternStep::Save
        );

        //assert!(selection.done());
    }

    #[test]
    fn test_selection_fsm_complex_element_selection_with_step_back() {
        let mut reader = Reader::new("p.indent > .bold");
        let pattern = PatternSection::new(&mut reader, Mode::All);
        let builder = Pattern::new(Vec::from([pattern]));
        let mut selection = Selection::new();

        let mut fsm_moved: bool = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
            &builder.list,
            &XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some("bold"),
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(
            builder.list[selection.pattern].pattern[selection.position],
            PatternStep::Save
        );

        //assert!(selection.done());

        selection.back(&builder.list, 1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        // Does nothing
        selection.back(&builder.list, 1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        selection.back(&builder.list, 0);

        assert_eq!(selection.is_reset(), true);
    }
}
