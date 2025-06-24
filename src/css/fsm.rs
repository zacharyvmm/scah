use super::query::QueryKind;
use crate::utils::reader::Reader;
use crate::{css::query::Combinator, xhtml::element::element::XHtmlElement};

pub struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
    positon: usize,
}

impl<'a> From<&mut Reader<'a>> for Selection<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut selection = Self {
            query: Vec::new(),
            positon: 0,
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
            positon: 0,
        }
    }

    fn check_previous_selector_validity(&mut self, depth: u8) -> bool {
        let previous = &mut self.query[self.positon - 1];

        match previous {
            QueryKind::Element(element) => {
                return true;
            }

            QueryKind::Combinator(Combinator::Child(previous_element_depth)) => {
                if *previous_element_depth != depth - 1 {
                    return false;
                }

                return true;
            }

            QueryKind::Combinator(Combinator::NextSibling(previous_sibling)) => {
                if *previous_sibling == true {
                    *previous_sibling = false;
                    return false;
                }

                return true;
            }

            QueryKind::Combinator(_) => {
                self.positon += 1;

                return true;
            }
            QueryKind::EOF => false,
            _ => true,
        }
    }

    pub fn transition<'b>(&mut self, xhtml_element: &XHtmlElement<'b>, depth: u8) -> bool {
        let current = &mut self.query[self.positon];

        match current {
            QueryKind::Element(element) => {
                if element == xhtml_element {
                    self.positon += 1;

                    return true;
                }

                return false;
            }

            QueryKind::Combinator(Combinator::Child(previous_element_depth)) => {
                *previous_element_depth = depth;
                self.positon += 1;

                return true;
            }

            QueryKind::Combinator(Combinator::NextSibling(previous_sibling)) => {
                *previous_sibling = true;
                self.positon += 1;

                return true;
            }

            QueryKind::Combinator(Combinator::SubsequentSibling(subsequent)) => {
                *subsequent = true;
                self.positon += 1;

                return true;
            }

            QueryKind::Combinator(_) => {
                self.positon += 1;

                return true;
            }
            QueryKind::EOF => true,
            _ => false,
        }
    }

    // TODO: Step back:
    // If the fsm have moved already then this function is to do a reverse transition when the
    // element is out of scope but the Selection have not been created.
    pub fn step_back(&mut self) {}
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
        let selection = Selection::from(&mut reader);
    }
}
