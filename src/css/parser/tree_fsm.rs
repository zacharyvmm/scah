use super::query_tokenizer::Combinator;
use super::query_tokenizer::{QueryKind, ChainKind};
use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

#[derive(Debug, Clone)]
pub struct TreeSelection<'a> {
    query: Vec<QueryKind<'a>>,
    position: usize,
}

impl<'a> From<&mut Reader<'a>> for TreeSelection<'a> {
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

impl<'a> TreeSelection<'a> {
    pub fn new() -> Self {
        Self {
            query: Vec::new(),
            position: 0,
        }
    }

    pub fn append(&mut self, mut other: Self, chain: Option<ChainKind>) -> () {

        // Operation
        // x -> o
        //   -> o

        // List
        // x -> o o o o o o o


        // remove blank first element
        other.query.remove(0);

        if chain != None {
            self.query.pop();
            self.query.push(QueryKind::Offset(chain, other.query.len() as u16));
        }

        self.query.append(&mut other.query);
    }
}

#[cfg(test)]
mod tests {
    use super::super::element::element::QueryElement;
    use super::*;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let selection = TreeSelection::from(&mut reader);

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
    fn test_selection_on_complex_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let mut then_a_reader = Reader::new("> a");
        let mut then_p_reader = Reader::new(" p");

        let mut selection = TreeSelection::from(&mut reader);

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

        selection.append(TreeSelection::from(&mut then_a_reader), Some(ChainKind::And));
        selection.append(TreeSelection::from(&mut then_p_reader), None);

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
                QueryKind::Offset(Some(ChainKind::And), 3),
                QueryKind::Combinator(Combinator::Child),
                QueryKind::Element(
                    0,
                    QueryElement::new(
                        Some("a"),
                        None,
                        None,
                        Vec::new(),
                    )
                ),
                QueryKind::EOF,
                QueryKind::Combinator(Combinator::Descendant),
                QueryKind::Element(
                    0,
                    QueryElement::new(
                        Some("p"),
                        None,
                        None,
                        Vec::new(),
                    )
                ),
                QueryKind::EOF,
            ])
        );

    }
}
