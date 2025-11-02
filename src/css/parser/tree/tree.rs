use super::super::fsm::Fsm;
use super::selection::SelectionKind;
use super::selection::SelectionPart;
use crate::css::parser::element::QueryElement;

#[derive(PartialEq, Debug, Clone)]
pub struct Position {
    pub(crate) section: usize,
    pub(crate) fsm: usize,
}

#[derive(PartialEq, Debug)]
pub enum NextPosition {
    Link(Position),
    Fork(Vec<Position>),
    EndOfBranch,
}

impl NextPosition {
    pub fn unwrap_link(self, error_message: &str) -> Position {
        match self {
            Self::Link(position) => position,
            _ => panic!("{}", error_message),
        }
    }
}

pub struct SelectionTree<'query> {
    pub list: Vec<SelectionPart<'query>>,
}

impl<'query> SelectionTree<'query> {
    pub fn new(list: Vec<SelectionPart<'query>>) -> Self {
        Self { list: list }
    }

    pub fn append(&mut self, mut sections: Vec<SelectionPart<'query>>) -> () {
        let list_len = self.list.len();
        if list_len > 0 && sections.len() > 1 {
            let ref mut last = self.list[list_len - 1];

            for index in 0..sections.len() {
                if sections[index].parent == Option::None {
                    sections[index].parent = Some(list_len - 1);
                    last.children.push(index);
                }
            }
        }

        self.list.append(&mut sections);
    }

    pub fn get(&self, position: &Position) -> &Fsm<'query> {
        return &self.list[position.section].fsms[position.fsm];
    }

    pub fn get_section_selection_kind(&self, section_index: usize) -> &SelectionKind {
        assert!(section_index < self.list.len());
        return &self.list[section_index].kind;
    }

    pub fn is_save_point(&self, position: &Position) -> bool {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        // if it's the last fsm in the section
        return position.fsm == self.list[position.section].len() - 1;
    }

    pub fn next(&self, position: &Position) -> NextPosition {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        if position.fsm + 1 < self.list[position.section].len() {
            return NextPosition::Link(Position {
                section: position.section,
                fsm: position.fsm + 1,
            });
        } else {
            let ref section = self.list[position.section];
            if section.children.len() == 0 {
                return NextPosition::EndOfBranch;
            }

            return NextPosition::Fork(
                section
                    .children
                    .iter()
                    .map(|child_index| Position {
                        section: *child_index,
                        fsm: 0,
                    })
                    .collect(),
            );
        }
    }

    pub fn back(&self, position: &Position) -> Position {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        if position.fsm > 0 {
            return Position {
                section: position.section,
                fsm: position.fsm - 1,
            };
        } else {
            let ref section = self.list[position.section];
            if let Some(parent) = section.parent {
                return Position {
                    section: parent,
                    fsm: section.len() - 1,
                };
            } else {
                return Position { section: 0, fsm: 0 };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::lexer::Combinator;
    use super::super::selection::{Save, SelectionKind};
    use super::*;
    use crate::css::parser::element::QueryElement;
    use crate::utils::Reader;

    #[test]
    fn test_selection_on_basic_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let section = SelectionPart::new(
            &mut reader,
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let tree = SelectionTree::new(Vec::from([section]));

        assert_eq!(
            tree.list[0].fsms,
            Vec::from([
                Fsm {
                    state: QueryElement::new(
                        Some("element"),
                        Some("id"),
                        Some("class"),
                        Vec::new(),
                    ),
                    transition: Combinator::Descendant,
                },
                Fsm {
                    state: QueryElement::new(
                        Some("other"),
                        Some("other_id"),
                        Some("other_class"),
                        Vec::new(),
                    ),
                    transition: Combinator::Child,
                }
            ])
        );
    }

    #[test]
    fn test_selection_on_complex_query() {
        let mut reader = Reader::new("element#id.class > other#other_id.other_class");
        let mut then_a_reader = Reader::new("> a");
        let mut then_p_reader = Reader::new(" p");

        let section = SelectionPart::new(
            &mut reader,
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let mut selection = SelectionTree::new(Vec::from([section]));

        assert_eq!(
            selection.list,
            Vec::from([SelectionPart {
                fsms: Vec::from([
                    Fsm {
                        state: QueryElement::new(
                            Some("element"),
                            Some("id"),
                            Some("class"),
                            Vec::new(),
                        ),
                        transition: Combinator::Descendant
                    },
                    Fsm {
                        state: QueryElement::new(
                            Some("other"),
                            Some("other_id"),
                            Some("other_class"),
                            Vec::new(),
                        ),
                        transition: Combinator::Child
                    },
                ]),
                descendants: Vec::new(),
                kind: SelectionKind::All(Save {
                    inner_html: false,
                    text_content: false
                }),
                parent: Option::None,
                children: Vec::new(),
            }])
        );

        selection.append(Vec::from([
            SelectionPart::new(
                &mut then_a_reader,
                SelectionKind::All(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
            SelectionPart::new(
                &mut then_p_reader,
                SelectionKind::All(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
        ]));

        assert_eq!(
            selection.list,
            Vec::from([
                SelectionPart {
                    fsms: Vec::from([
                        Fsm {
                            state: QueryElement::new(
                                Some("element"),
                                Some("id"),
                                Some("class"),
                                Vec::new(),
                            ),
                            transition: Combinator::Descendant
                        },
                        Fsm {
                            state: QueryElement::new(
                                Some("other"),
                                Some("other_id"),
                                Some("other_class"),
                                Vec::new(),
                            ),
                            transition: Combinator::Child
                        },
                    ]),
                    descendants: Vec::new(),
                    kind: SelectionKind::All(Save {
                        inner_html: false,
                        text_content: false
                    }),
                    parent: Option::None,
                    children: Vec::from([0, 1]),
                },
                SelectionPart {
                    fsms: Vec::from([Fsm {
                        state: QueryElement::new(Some("a"), None, None, Vec::new(),),
                        transition: Combinator::Child
                    },]),
                    descendants: Vec::new(),
                    kind: SelectionKind::All(Save {
                        inner_html: false,
                        text_content: false
                    }),
                    parent: Some(0),
                    children: Vec::new(),
                },
                SelectionPart {
                    fsms: Vec::from([Fsm {
                        state: QueryElement::new(Some("p"), None, None, Vec::new(),),
                        transition: Combinator::Descendant
                    },]),
                    descendants: Vec::new(),
                    kind: SelectionKind::All(Save {
                        inner_html: false,
                        text_content: false
                    }),
                    parent: Some(0),
                    children: Vec::new(),
                },
            ])
        );
    }
}
