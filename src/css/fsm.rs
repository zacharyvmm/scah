use super::element::element::Element;
use super::parser::{Combinator, QueryKind};
use crate::utils::reader::Reader;
// Build FSM from css selector

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

        while let Some(query) = QueryKind::next(reader, selection.query.last()) {
            selection.query.push(query);
        }

        return selection;
    }
}

impl<'a> Selection<'a> {
    pub fn transition(&mut self, other: &Element<'a>) -> () {
        let current = &self.query[self.positon];

        match current {
            QueryKind::Element(element) => {}

            // TODO: This based on this we either 1) move or 2) stay put or 3) give up
            QueryKind::Combinator(combinator) => {}
            _ => (),
        }
    }

    // TODO: Step back:
    // If the fsm have moved already then this function is to do a reverse transition when the
    // element is out of scope but the Selection have not been created.
    pub fn step_back(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let selection = Selection::from(&mut reader);

        assert_eq!(
            selection.query[0],
            QueryKind::Element(Element::new(
                Some("element"),
                Some("id"),
                Some("class"),
                Vec::new(),
            ))
        );

        assert_eq!(selection.query[1], QueryKind::Combinator(Combinator::Child));

        assert_eq!(
            selection.query[2],
            QueryKind::Element(Element::new(
                Some("other"),
                Some("other_id"),
                Some("other_class"),
                Vec::new(),
            ))
        );
    }

    #[test]
    fn test_selection_fsm_element_name_selection() {
        let mut reader = Reader::new("element > p");
        let selection = Selection::from(&mut reader);
    }
}
