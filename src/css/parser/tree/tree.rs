use smallvec::SmallVec;
use smallvec::smallvec;

use super::super::fsm::Fsm;
use super::super::lexer::Combinator;
use super::selection::SelectionKind;
use super::selection::SelectionPart;

#[derive(PartialEq, Debug, Clone)]
pub struct Position {
    pub(crate) section: usize,
    pub(crate) fsm: usize,
}

pub(crate) type NextPositions = SmallVec<[Position; 6]>;  

#[derive(PartialEq, Debug)]
pub enum NextPosition {
    Link(Position),
    Fork(NextPositions),
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
#[derive(Debug)]
pub struct Query<'query> {
    list: Box<[SelectionPart<'query>]>
}

impl<'query> Query<'query> {
    pub fn get(&self, position: &Position) -> &Fsm<'query> {
        &self.list[position.section].fsms[position.fsm]
    }

    pub fn get_section_selection_kind(&self, section_index: usize) -> &SelectionKind {
        assert!(section_index < self.list.len());
        &self.list[section_index].kind
    }

    pub fn get_section(&self, section_index: usize) -> &SelectionPart<'query> {
        assert!(section_index < self.list.len());
        &self.list[section_index]
    }

    pub fn is_descendant(&self, position: &Position) -> bool {
        self.get(position).transition == Combinator::Descendant
    }

    pub fn is_save_point(&self, position: &Position) -> bool {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        // if it's the last fsm in the section
        position.fsm == self.list[position.section].len() - 1
    }
    pub fn is_last_save_point(&self, position: &Position) -> bool {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        // if it's the last fsm in the section
        let last_section = self.list.len() - 1;
        let last_element_of_last_section = self.list[last_section].len() - 1;

        let is_last_section = position.section == last_section;
        let is_last_element_of_last_section = position.fsm == last_element_of_last_section;

        is_last_section && is_last_element_of_last_section
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
                    .map(|child_offset| Position {
                        section: *child_offset + position.section,
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

#[derive(Debug)]
pub struct QueryBuilder<'query> {
    pub list: Vec<SelectionPart<'query>>,
}

impl<'query> QueryBuilder<'query> {
    pub fn new(root_element: SelectionPart<'query>) -> Self {
        Self {
            list: vec![root_element],
        }
    }

    pub fn append(&mut self, mut sections: Vec<SelectionPart<'query>>) {
        let list_len = self.list.len();
        if list_len > 0 && sections.len() > 1 {
            let last_element_index = list_len - 1;
            let ref mut last = self.list[last_element_index];

            for index in 0..sections.len() {
                if sections[index].parent.is_none() {
                    sections[index].parent = Some(last_element_index);
                    last.children.push(index + 1);
                }
            }
        }

        self.list.append(&mut sections);
    }

    pub fn build(self) -> Query<'query> {
        Query { list: self.list.into_boxed_slice() }
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
        let section = SelectionPart::new(
            "element#id.class > other#other_id.other_class",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let tree = QueryBuilder::new(section);

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
        let section = SelectionPart::new(
            "element#id.class > other#other_id.other_class",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let mut selection = QueryBuilder::new(section);

        assert_eq!(
            selection.list,
            Vec::from([SelectionPart {
                source: "element#id.class > other#other_id.other_class",
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
                "> a",
                SelectionKind::All(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
            SelectionPart::new(
                " p",
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
                    source: "element#id.class > other#other_id.other_class",
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
                    children: Vec::from([1, 2]),
                },
                SelectionPart {
                    source: "> a",
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
                    source: " p",
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
