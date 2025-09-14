use super::query_tokenizer::Combinator;
use super::query_tokenizer::QueryKind;
use crate::css::parser::element::element::QueryElement;
use crate::utils::reader::Reader;
use crate::xhtml::element::element::XHtmlElement;

#[derive(PartialEq, Debug)]
enum Mode {
    First,
    All,
}

#[derive(PartialEq, Debug)]
struct QuerySection<'a> {
    list: Vec<QueryKind<'a>>,
    kind: Mode,

    parent: Option<u16>, // real index
    children: Vec<u16>,  // offset
}

impl<'a> QuerySection<'a> {
    fn new(reader: &mut Reader<'a>, mode: Mode) -> Self {
        let mut selection = Self {
            list: Vec::new(),
            kind: mode,
            parent: None,
            children: Vec::new(),
        };

        while let Some(token) = QueryKind::next(reader, selection.list.last()) {
            selection.list.push(token);
        }
        selection.list.push(QueryKind::Save);

        return selection;
    }
}

pub struct FsmBuilder<'a> {
    list: Vec<QuerySection<'a>>,
}

impl<'a> FsmBuilder<'a> {
    pub fn new(sections: Vec<QuerySection<'a>>) -> Self {
        Self { list: sections }
    }

    pub fn append(&mut self, mut sections: Vec<QuerySection<'a>>) -> () {
        let list_len = self.list.len();
        if list_len > 0 && sections.len() > 1 {
            let ref mut last = self.list[list_len - 1];

            for index in 0..sections.len() {
                // ----- Empty element -----
                assert!(matches!(
                    sections[index].list[0],
                    QueryKind::Element(
                        _,
                        QueryElement {
                            name: None | Some("&"),
                            ..
                        }
                    )
                ));
                sections[index].list.remove(0);
                // -------------------------

                if sections[index].parent == Option::None {
                    sections[index].parent = Some((list_len - 1) as u16);
                    last.children.push(index as u16);
                }
            }
        }

        self.list.append(&mut sections);
    }
}

#[cfg(test)]
mod tests {
    use super::super::element::element::QueryElement;
    use super::*;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let section = QuerySection::new(&mut reader, Mode::All);
        let selection = FsmBuilder::new(Vec::from([section]));

        assert_eq!(
            selection.list[0].list,
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

        let section = QuerySection::new(&mut reader, Mode::All);
        let mut selection = FsmBuilder::new(Vec::from([section]));

        assert_eq!(
            selection.list,
            Vec::from([QuerySection {
                list: Vec::from([
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
                ]),
                kind: Mode::All,
                parent: Option::None,
                children: Vec::new(),
            }])
        );

        selection.append(Vec::from([
            QuerySection::new(&mut then_a_reader, Mode::All),
            QuerySection::new(&mut then_p_reader, Mode::All),
        ]));

        assert_eq!(
            selection.list,
            Vec::from([
                QuerySection {
                    list: Vec::from([
                        QueryKind::Element(
                            0,
                            QueryElement::new(
                                Some("element"),
                                Some("id"),
                                Some("class"),
                                Vec::new(),
                            )
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
                    ]),
                    kind: Mode::All,
                    parent: Option::None,
                    children: Vec::from([0, 1]),
                },
                QuerySection {
                    list: Vec::from([
                        QueryKind::Combinator(Combinator::Child),
                        QueryKind::Element(
                            0,
                            QueryElement::new(Some("a"), None, None, Vec::new(),)
                        ),
                        QueryKind::Save,
                    ]),
                    kind: Mode::All,
                    parent: Some(0),
                    children: Vec::new(),
                },
                QuerySection {
                    list: Vec::from([
                        QueryKind::Combinator(Combinator::Descendant),
                        QueryKind::Element(
                            0,
                            QueryElement::new(Some("p"), None, None, Vec::new(),)
                        ),
                        QueryKind::Save,
                    ]),
                    kind: Mode::All,
                    parent: Some(0),
                    children: Vec::new(),
                },
            ])
        );
    }
}
