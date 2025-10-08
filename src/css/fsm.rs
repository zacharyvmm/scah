use super::parser::pattern::Pattern;
use super::parser::query_tokenizer::Combinator;
use super::parser::query_tokenizer::PatternStep;
use crate::css::parser::pattern::NextPosition;
use crate::xhtml::element::element::XHtmlElement;

#[derive(Debug, Clone)]
pub struct Selection {
    element_depth_stack: Vec<usize>,
    pub(super) pattern: usize,
    pub(super) position: usize,
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

    fn skip_until_next_element<'query>(&mut self, query: &Pattern<'query>) {
        let (new_pattern, new_position) = query
            .next(self.pattern, self.position)
            .unwrap_link("It's should not be possible to be a fork here");

        let current = query.get(new_pattern, new_position);

        if let PatternStep::Combinator(_) = current {
            (self.pattern, self.position) = (new_pattern, new_position);
        }
    }

    fn previous_child_combinator_invalid<'query>(&mut self, query: &Pattern, depth: usize) -> bool {
        if self.position == 0 {
            return false;
        }

        let (p1, p2) = query.back(self.pattern, self.position);
        let previous = query.get(p1, p2);

        // assert: not Element
        println!("{} {}", p1, p2);
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
    fn next_sibling_failed_move_back<'query>(&mut self, query: &Pattern<'query>) {
        if self.pattern == 0 && self.position == 0 {
            return;
        }

        let (p1, p2) = query.back(self.pattern, self.position);
        let previous = query.get(p1, p2);

        if let PatternStep::Combinator(Combinator::NextSibling) = previous {
            (self.pattern, self.position) = query.back(self.pattern, self.position);
            assert!(matches!(
                query.get(self.pattern, self.position),
                &PatternStep::Element(..)
            ));
            (self.pattern, self.position) = query.back(self.pattern, self.position);
            if *query.get(self.pattern, self.position) == PatternStep::Save {
                (self.pattern, self.position) = query.back(self.pattern, self.position);
            }

            assert!(matches!(
                query.get(self.pattern, self.position),
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
        query: &Pattern<'query>,
        xhtml_element: &XHtmlElement<'html>,
        stack_depth: usize,
    ) -> NextPosition {
        if self.previous_child_combinator_invalid(&query, stack_depth) {
            return NextPosition::NoMovement;
        }

        // TODO: Refactor this to a match to handle `Has` and `Not`

        if let PatternStep::Element(element) = query.get(self.pattern, self.position) {
            // NOTE: Compare xhtml element to selector element
            if element == xhtml_element {
                self.element_depth_stack.push(stack_depth);
                self.skip_until_next_element(query);
                return query.next(self.pattern, self.position);
            }

            self.next_sibling_failed_move_back(query);
        }
        return NextPosition::NoMovement;
    }

    pub fn back<'query>(&mut self, query: &Pattern<'query>, depth: usize) {
        if self.position == 0 {
            return;
        }

        if *query.get(self.pattern, self.position) == PatternStep::Save {
            if self.position == 0 {
                self.pattern -= 1;
            } else {
                self.position -= 1;
            }

            assert!(
                matches!(
                    *query.get(self.pattern, self.position),
                    PatternStep::Element(..)
                ),
                "The item in the query list before a child combinator should always be an `Element`."
            );
            self.element_depth_stack.pop();
            return;
        }

        let mut element_pattern = self.pattern;
        let mut element_position = self.position;
        //assert!(!matches!(self.query[element_position], PatternStep::Element(..)));
        if !matches!(
            *query.get(self.pattern, self.position),
            PatternStep::Element(..)
        ) {
            (element_pattern, element_position) = query.back(self.pattern, self.position);
        }

        assert!(matches!(
            *query.get(element_pattern, element_position),
            PatternStep::Element(..)
        ));

        let element_depth = self
            .element_depth_stack
            .last()
            .expect("Must have an element in the stack");

        if *element_depth == depth {
            //assert!(element_position >= 2);
            //self.position = element_position - 2;
            (self.pattern, self.position) = query.back(element_pattern, element_position);
            (self.pattern, self.position) = query.back(self.pattern, self.position);
            self.element_depth_stack.pop();

            assert!(matches!(
                *query.get(self.pattern, self.position),
                PatternStep::Element(..)
            ));
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

        println!("1");
        let mut fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "element",
                    id: None,
                    class: None,
                    attributes: Vec::new(),
                },
                0,
            )
            .unwrap_link("has to be a link here");
        println!("2");

        // Manually move
        (selection.pattern, selection.position) = fsm_moved;

        println!(
            "{} {} {:?}",
            selection.pattern,
            selection.position,
            *builder.get(selection.pattern, selection.position)
        );

        println!("3");
        assert_eq!(
            selection.next(
                &builder,
                &XHtmlElement {
                    name: "p",
                    id: None,
                    class: None,
                    attributes: Vec::new(),
                },
                0,
            ),
            NextPosition::NoMovement
        );
        println!("4");

        fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "p",
                    id: None,
                    class: None,
                    attributes: Vec::new(),
                },
                1,
            )
            .unwrap_link("has to be a link here");
        println!("5");

        // Manually move
        (selection.pattern, selection.position) = fsm_moved;

        assert_eq!(
            *builder.get(selection.pattern, selection.position),
            PatternStep::Save
        );
    }

    #[test]
    fn test_selection_fsm_complex_element_selection() {
        let mut reader = Reader::new("p.indent > .bold");
        let pattern = PatternSection::new(&mut reader, Mode::All);
        let builder = Pattern::new(Vec::from([pattern]));
        let mut selection = Selection::new();

        let mut fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "p",
                    id: None,
                    class: Some("indent"),
                    attributes: Vec::new(),
                },
                0,
            )
            .unwrap_link("has to be a link here");

        (selection.pattern, selection.position) = fsm_moved;

        fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "span",
                    id: Some("name"),
                    class: Some("bold"),
                    attributes: Vec::new(),
                },
                1,
            )
            .unwrap_link("has to be a link here");

        (selection.pattern, selection.position) = fsm_moved;

        assert_eq!(
            *builder.get(selection.pattern, selection.position),
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

        let mut fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "p",
                    id: None,
                    class: Some("indent"),
                    attributes: Vec::new(),
                },
                0,
            )
            .unwrap_link("has to be a link here");

        (selection.pattern, selection.position) = fsm_moved;

        fsm_moved = selection
            .next(
                &builder,
                &XHtmlElement {
                    name: "span",
                    id: Some("name"),
                    class: Some("bold"),
                    attributes: Vec::new(),
                },
                1,
            )
            .unwrap_link("has to be a link here");

        (selection.pattern, selection.position) = fsm_moved;

        assert_eq!(
            *builder.get(selection.pattern, selection.position),
            PatternStep::Save
        );

        //assert!(selection.done());

        selection.back(&builder, 1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        // Does nothing
        selection.back(&builder, 1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        selection.back(&builder, 0);

        assert_eq!(selection.is_reset(), true);
    }
}
