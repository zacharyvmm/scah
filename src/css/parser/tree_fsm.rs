use super::query_tokenizer::Combinator;
use super::query_tokenizer::QueryKind;
use crate::css::parser::element::element::QueryElement;
use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

#[derive(PartialEq)]
enum Mode {
    First,
    All,
}

enum Store<'a> {
    Single(Option<XHtmlElement<'a>>),
    List1D(Vec<XHtmlElement<'a>>),
    List2D(Vec<Vec<XHtmlElement<'a>>>),
    List3D(Vec<Vec<Vec<XHtmlElement<'a>>>>),
}

pub struct TreeBuilder<'a> {
    max_save_depth: u8,
    previous_save_depth: u8,
    mode: Mode,
    list: Vec<QueryKind<'a>>,
}

impl<'a> From<&mut Reader<'a>> for TreeBuilder<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut selection = Self { 
            max_save_depth: 1,
            previous_save_depth: 0,
            mode: Mode::All,
            list: Vec::new()};

        while let Some(token) = QueryKind::next(reader, selection.list.last()) {
            selection.list.push(token);
        }
        selection.list.push(QueryKind::Save);

        return selection;
    }
}

impl<'a> TreeBuilder<'a> {

    pub fn append(&mut self, branch: bool, mut other: Self) -> () {
        // Operation
        // x -> o
        //   -> o

        // List
        // x -> o o o o o o o

        // [blank element, combinator, ...]
        // remove blank first element
        assert!(matches!(
            other.list[0],
            QueryKind::Element(
                _,
                QueryElement {
                    name: None | Some("&"),
                    ..
                }
            )
        ));
        other.list.remove(0);

        if branch {
            // Only if I don't want to save the end element of the previous query
            self.list.pop();

            self.list
                .push(QueryKind::Offset(other.list.len() as u16));

            self.previous_save_depth = other.max_save_depth;
            self.max_save_depth += other.max_save_depth;

            other.max_save_depth = 0;
        }

        if self.previous_save_depth < other.max_save_depth {
            self.max_save_depth = self.max_save_depth - self.previous_save_depth + other.max_save_depth;
            self.previous_save_depth = 0;
        }

        self.list.append(&mut other.list);
    }

    fn mode(&mut self, mode: Mode) -> () {
        self.mode = mode;
    }

    fn storage(&self) -> Store<'a> {
        
        match self.mode {
            Mode::First => {
                if self.max_save_depth == 1 {
                    return Store::Single(None);
                }

                return Store::List1D(Vec::with_capacity(self.max_save_depth as usize));
            },
            Mode::All => {
                if self.max_save_depth == 1 {
                    return Store::List1D(Vec::new());
                }

                return Store::List2D(Vec::from([Vec::with_capacity(self.max_save_depth as usize)]))
            },
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
        let selection = TreeBuilder::from(&mut reader);

        assert_eq!(
            selection.list,
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
                QueryKind::Save,
            ])
        );
    }

    #[test]
    fn test_selection_on_complex_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let mut then_a_reader = Reader::new("> a");
        let mut then_p_reader = Reader::new(" p");

        let mut selection = TreeBuilder::from(&mut reader);

        assert_eq!(
            selection.list,
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
                QueryKind::Save,
            ])
        );

        selection.append(true, TreeBuilder::from(&mut then_a_reader));
        selection.append(true, TreeBuilder::from(&mut then_p_reader));

        assert_eq!(
            selection.list,
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
                QueryKind::Offset(3),
                QueryKind::Combinator(Combinator::Child),
                QueryKind::Element(0, QueryElement::new(Some("a"), None, None, Vec::new(),)),
                QueryKind::Save,
                QueryKind::Combinator(Combinator::Descendant),
                QueryKind::Element(0, QueryElement::new(Some("p"), None, None, Vec::new(),)),
                QueryKind::Save,
            ])
        );
    }
}
