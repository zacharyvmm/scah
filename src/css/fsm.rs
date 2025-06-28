use super::query::QueryKind;
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

    fn next_until_element<'b>(&mut self, depth: u8) {
        let current = &self.query[self.position];
        if let QueryKind::Combinator(_) = current {
            self.position += 1;
        }
    }

    fn previous_child_combinator_invalid(&mut self, depth: u8) -> bool {
        if self.position == 0 {
            return false;
        }

        let previous = &self.query[self.position - 1];

        assert!(!matches!(previous, &QueryKind::Element(..)));

        if let QueryKind::Combinator(Combinator::Child(previous_element_depth)) = previous {
            if *previous_element_depth + 1 != depth {
                return true;
            }
            return false;
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

    pub fn reset(&mut self) {
        self.position = 0;

        for query in &mut self.query {
            if let QueryKind::Combinator(Combinator::Child(previous_element_depth)) = query {
                *previous_element_depth = 0;
            }
        }
    }

    pub fn next<'b>(&mut self, xhtml_element: &XHtmlElement<'b>, depth: u8) -> bool {
        self.next_until_element(depth);

        if self.previous_child_combinator_invalid(depth) {
            return false;
        }

        let current = &self.query[self.position];

        // TODO: Refactor this to a match to handle `Has` and `Not`
        if let QueryKind::Element(element) = current {
            // NOTE: Compare xhtml element to selector element
            if element == xhtml_element {
                self.position += 1;
                println!("increase position");
                let next = &mut self.query[self.position];
                if let QueryKind::Combinator(Combinator::Child(previous_element_depth)) = next {
                    if *previous_element_depth == 0 {
                        *previous_element_depth = depth;
                    }
                }

                return true;
            }

            self.next_sibling_failed_move_back();

            return false;
        }
        return false;
    }

    // TODO: Step back:
    // If the fsm have moved already then this function is to do a reverse transition when the
    // element is out of scope but the Selection have not been created.
    pub fn back(&mut self, depth: u8) {}
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
                QueryKind::Element(QueryElement::new(
                    Some("element"),
                    Some("id"),
                    Some("class"),
                    Vec::new(),
                )),
                QueryKind::Combinator(Combinator::Child(0)),
                QueryKind::Element(QueryElement::new(
                    Some("other"),
                    Some("other_id"),
                    Some("other_class"),
                    Vec::new(),
                )),
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

        assert_eq!(fsm_moved, true);
        assert_eq!(
            selection.query[selection.position],
            QueryKind::Combinator(Combinator::Child(0))
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

        assert_eq!(fsm_moved, true);
        assert_eq!(
            selection.query[selection.position],
            QueryKind::Combinator(Combinator::Child(0))
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
}
