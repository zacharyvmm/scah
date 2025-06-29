use super::query::QueryKind;
use crate::css::element::element::QueryElement;
use crate::css::query::Combinator;
use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

#[derive(Debug, Clone)]
pub struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
    position: usize,
}

impl<'a> From<&mut Reader<'a>> for Selection<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut selection = Self {
            query: Vec::new(),
            position: 0,
        };

        while let Some(token) = QueryKind::next(reader, selection.query.last()) {
            selection.query.push(token);
        }
        selection.query.push(QueryKind::EOF);

        return selection;
    }
}

impl<'a> Selection<'a> {
    pub fn new() -> Self {
        Self {
            query: Vec::new(),
            position: 0,
        }
    }

    fn next_until_element<'b>(&mut self) {
        let current = &self.query[self.position];
        if let QueryKind::Combinator(_) = current {
            self.position += 1;
        }
        assert!(matches!(
            self.query[self.position],
            QueryKind::Element(..) | QueryKind::EOF
        ));
    }

    fn previous_child_combinator_invalid(&mut self, depth: u8) -> bool {
        if self.position == 0 {
            return false;
        }

        let previous = &self.query[self.position - 1];

        assert!(!matches!(previous, &QueryKind::Element(..)));

        if let QueryKind::Combinator(Combinator::Child) = previous {
            let previous_element = &mut self.query[self.position - 2];
            let QueryKind::Element(element_depth, _) = previous_element else {
                panic!(
                    "The item in the query list before a child combinator should always be an `Element`."
                )
            };

            return *element_depth + 1 != depth;
        } else {
            false
        }
    }

    // NOTE: In the case of failure IF last element is NextSibling, then go back to the Element that the NextSibling is referencing
    fn next_sibling_failed_move_back(&mut self) {
        if self.position == 0 {
            return;
        }

        let previous = &self.query[self.position - 1];

        if let QueryKind::Combinator(Combinator::NextSibling) = previous {
            self.position -= 2;
            assert!(matches!(
                &self.query[self.position],
                &QueryKind::Element(..)
            ));
        }
    }

    pub fn done(&self) -> bool {
        assert!(self.position <= self.query.len());

        self.query[self.position] == QueryKind::EOF
    }

    pub fn is_reset(&self) -> bool {
        assert!(self.position <= self.query.len());

        self.position == 0
    }

    pub fn reset(&mut self) {
        self.position = 0;

        for query in &mut self.query {
            if let QueryKind::Element(depth, _) = query {
                *depth = 0;
            }
        }
    }

    pub fn next<'b>(&mut self, xhtml_element: &XHtmlElement<'b>, stack_depth: u8) -> bool {
        if self.previous_child_combinator_invalid(stack_depth) {
            return false;
        }

        // TODO: Refactor this to a match to handle `Has` and `Not`

        println!("LIST: {:?}", self.query);
        if let QueryKind::Element(ref mut depth, ref element) = self.query[self.position] {
            println!("Comparing `{:?}` with `{:?}`", element, xhtml_element);
            // NOTE: Compare xhtml element to selector element
            if element == xhtml_element {
                self.position += 1;

                *depth = stack_depth;
                println!("LIST: {:?}", self.query);

                self.next_until_element();
                return true;
            }

            self.next_sibling_failed_move_back();

            return false;
        }
        return false;
    }

    pub fn back(&mut self, depth: u8) {
        if self.position == 0 {
            return;
        }

        if self.query[self.position] == QueryKind::EOF {
            println!("EOF {}", self.position);
            self.position -= 1;
            let QueryKind::Element(ref mut element_depth, _) = self.query[self.position] else {
                panic!(
                    "The item in the query list before a child combinator should always be an `Element`."
                )
            };
            *element_depth = 0;
            return;
        }

        let mut element_position = self.position;
        //assert!(!matches!(self.query[element_position], QueryKind::Element(..)));
        if !matches!(self.query[element_position], QueryKind::Element(..)) {
            element_position = self.position - 1;
        }

        let element_depth = match &mut self.query[element_position] {
            QueryKind::Element(0, _) => {
                let previous_element = &mut self.query[element_position - 2];
                let QueryKind::Element(el_depth, _) = previous_element else {
                    panic!(
                        "The item in the query list before a child combinator should always be an `Element`."
                    )
                };

                el_depth
            }
            QueryKind::Element(el_depth, _) => el_depth,
            _ => panic!(
                "The item in the query list before a child combinator should always be an `Element`."
            ),
        };

        println!("{} == {}", *element_depth, depth);
        if *element_depth == depth {
            assert!(element_position >= 2);
            self.position = element_position - 2;
            *element_depth = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::element::element::QueryElement;
    use super::*;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let selection = Selection::from(&mut reader);

        assert_eq!(
            selection.query,
            Vec::from([
                QueryKind::Element(
                    0,
                    QueryElement::new(Some("element"), Some("id"), Some("class"), Vec::new(),)
                ),
                QueryKind::Combinator(Combinator::Child),
                QueryKind::Element(
                    0,
                    QueryElement::new(
                        Some("other"),
                        Some("other_id"),
                        Some("other_class"),
                        Vec::new(),
                    )
                ),
                QueryKind::EOF,
            ])
        );
    }

    #[test]
    fn test_selection_fsm_element_name_selection() {
        let mut reader = Reader::new("element > p");
        let mut selection = Selection::from(&mut reader);

        let mut fsm_moved: bool = selection.next(
            &XHtmlElement {
                name: "element",
                id: None,
                class: None,
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
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
            &XHtmlElement {
                name: "p",
                id: None,
                class: None,
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(selection.query[selection.position], QueryKind::EOF);
    }

    #[test]
    fn test_selection_fsm_complex_element_selection() {
        let mut reader = Reader::new("p.indent > .bold");
        let mut selection = Selection::from(&mut reader);

        let mut fsm_moved: bool = selection.next(
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
            &XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some("bold"),
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(selection.query[selection.position], QueryKind::EOF);

        assert!(selection.done());
    }

    #[test]
    fn test_selection_fsm_complex_element_selection_with_step_back() {
        let mut reader = Reader::new("p.indent > .bold");
        let mut selection = Selection::from(&mut reader);

        let mut fsm_moved: bool = selection.next(
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new(),
            },
            0,
        );

        fsm_moved = selection.next(
            &XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some("bold"),
                attributes: Vec::new(),
            },
            1,
        );

        assert_eq!(fsm_moved, true);

        assert_eq!(selection.query[selection.position], QueryKind::EOF);

        assert!(selection.done());

        selection.back(1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        // Does nothing
        selection.back(1);

        assert_eq!(selection.is_reset(), false);
        assert_eq!(selection.position, 2);

        selection.back(0);

        assert_eq!(selection.is_reset(), true);
    }
}
