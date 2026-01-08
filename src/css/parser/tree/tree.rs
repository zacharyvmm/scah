use crate::Save;
use crate::css::parser::tree::selection::QuerySection;

use super::super::fsm::Fsm;
use super::super::lexer::Combinator;
use super::selection::SelectionKind;
use super::selection::SelectionPart;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position {
    pub(crate) section: usize,
    pub(crate) fsm: usize,
}

pub(crate) type NextPositions = Vec<Position>;

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
    list: Box<[QuerySection<'query>]>,
    pub(crate) exit_at_section_end: Option<usize>,
}

impl<'query> Query<'query> {
    pub fn first(query: &'query str, save: Save) -> QueryBuilder<'query> {
        QueryBuilder {
            list: vec![SelectionPart::new(query, SelectionKind::First(save))],
        }
    }

    pub fn all(query: &'query str, save: Save) -> QueryBuilder<'query> {
        QueryBuilder {
            list: vec![SelectionPart::new(query, SelectionKind::All(save))],
        }
    }

    pub(crate) fn get(&self, position: &Position) -> &Fsm<'query> {
        &self.list[position.section].fsms[position.fsm]
    }

    pub(crate) fn get_section_selection_kind(&self, section_index: usize) -> &SelectionKind {
        assert!(section_index < self.list.len());
        &self.list[section_index].kind
    }

    pub(crate) fn get_section(&self, section_index: usize) -> &QuerySection<'query> {
        assert!(section_index < self.list.len());
        &self.list[section_index]
    }

    pub(crate) fn is_descendant(&self, position: &Position) -> bool {
        self.get(position).transition == Combinator::Descendant
    }

    pub(crate) fn is_save_point(&self, position: &Position) -> bool {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        // if it's the last fsm in the section
        position.fsm == self.list[position.section].len() - 1
    }
    pub(crate) fn is_last_save_point(&self, position: &Position) -> bool {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        // if it's the last fsm in the section
        let last_section = self.list.len() - 1;
        let last_element_of_last_section = self.list[last_section].len() - 1;

        let is_last_section = position.section == last_section;
        let is_last_element_of_last_section = position.fsm == last_element_of_last_section;

        is_last_section && is_last_element_of_last_section
    }

    pub(crate) fn next(&self, position: &Position) -> NextPosition {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        if position.fsm + 1 < self.list[position.section].len() {
            NextPosition::Link(Position {
                section: position.section,
                fsm: position.fsm + 1,
            })
        } else {
            let section = &self.list[position.section];
            if section.children.is_empty() {
                return NextPosition::EndOfBranch;
            }

            NextPosition::Fork(
                section
                    .children
                    .iter()
                    .map(|child_offset| Position {
                        section: *child_offset + position.section,
                        fsm: 0,
                    })
                    .collect(),
            )
        }
    }

    pub(crate) fn back(&self, position: &Position) -> Position {
        assert!(position.section < self.list.len());
        assert!(position.fsm < self.list[position.section].len());

        if position.fsm > 0 {
            Position {
                section: position.section,
                fsm: position.fsm - 1,
            }
        } else {
            let section = &self.list[position.section];
            if let Some(parent) = section.parent {
                Position {
                    section: parent,
                    fsm: section.len() - 1,
                }
            } else {
                Position { section: 0, fsm: 0 }
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

    pub fn append(&mut self, parent: usize, mut sections: Vec<SelectionPart<'query>>) {
        let last = &mut self.list[parent];
        let mut offset = 0;

        for (index, section) in sections.iter_mut().enumerate() {
            let new_parent_index = match &section.parent {
                None => {
                    last.children.push(index + 1);
                    offset += 1;
                    parent
                }
                Some(p) => *p + offset + parent,
            };
            section.parent = Some(new_parent_index);
        }

        self.list.append(&mut sections);
    }

    pub fn all(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.list.is_empty());
        let mut part = SelectionPart::new(query, SelectionKind::All(save));
        part.parent = Some(self.list.len() - 1);
        self.list.push(part);
        self
    }

    pub fn first(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.list.is_empty());
        let mut part = SelectionPart::new(query, SelectionKind::First(save));
        part.parent = Some(self.list.len() - 1);
        self.list.push(part);
        self
    }

    pub fn then<F, I>(mut self, func: F) -> Self
    where
        F: FnOnce(QueryFactory) -> I,
        I: IntoIterator<Item = Self>,
    {
        let factory = QueryFactory {};
        let children = func(factory);

        let parts_vec_iter = children.into_iter().flat_map(|child| child.list);

        let current_index = self.list.len() - 1;
        self.append(current_index, parts_vec_iter.collect());
        self
    }

    fn exit_at_section(&self) -> Option<usize> {
        // returns the position in the selection tree where it can early exit
        // TODO: I should add a required flag for QuerySections, so that the first selection is nulled
        //  -> Basicly you can't return the first section without a perticular section behind added
        //  -> If you come back to the section without saving the required section,
        //      then you delete the saved data and you start over.

        fn search_for_single_exit_section(
            index: usize,
            list: &Vec<SelectionPart>,
        ) -> Option<usize> {
            // If you have a section with MULTIPLE children that can early exit,
            //   then this parent node will become the exit section
            if index >= list.len() {
                return None;
            }
            let section = &list[index];
            let stop_here = match &section.kind {
                //BUG: you can only early exit when the ALL of them have been found, thus the parent must be awaited for
                SelectionKind::All(_) => return None,

                // This is it need's to find the </{element}> to get either inner_html or text_content
                SelectionKind::First(save) => *save != Save::none(),
            };
            if stop_here {
                return Some(index);
            }

            if section.children.is_empty() {
                let child_can_early_exit = search_for_single_exit_section(index + 1, list);
                if child_can_early_exit.is_none() {
                    return Some(index);
                }
                return child_can_early_exit;
            }

            let mut child_response: Option<usize> = None;
            for child in &section.children {
                let child_can_early_exit = search_for_single_exit_section(index + *child, list);
                child_response = match child_response {
                    None => child_can_early_exit,
                    Some(_) => {
                        // If their's more than one child that can early exit then
                        // the parent is chosen
                        return Some(index);
                    }
                }
            }

            if child_response.is_some() {
                return child_response;
            }
            Some(index)
        }

        // BUG: I'm intentially adding this bug, because to actually solve this
        //  I would need to be able to check if all descandants in my fsm tree was saved to early exit
        search_for_single_exit_section(0, &self.list)
    }

    pub fn build(self) -> Query<'query> {
        let exit_at_section_end = self.exit_at_section();
        let list = self
            .list
            .into_iter()
            .map(|part| part.build())
            .collect::<Box<[QuerySection<'query>]>>();
        Query {
            list,
            exit_at_section_end,
        }
    }
}

#[derive(Debug)]
pub struct QueryFactory {}

impl<'query> QueryFactory {
    pub fn all(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        let part = SelectionPart::new(query, SelectionKind::All(save));
        QueryBuilder::new(part)
    }

    pub fn first(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        let part = SelectionPart::new(query, SelectionKind::First(save));
        QueryBuilder::new(part)
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
                kind: SelectionKind::All(Save {
                    inner_html: false,
                    text_content: false
                }),
                parent: Option::None,
                children: Vec::new(),
            }])
        );

        selection.append(
            0,
            Vec::from([
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
            ]),
        );

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

    #[test]
    fn test_then_based_query_builder() {
        let query = Query::first("section", Save::default())
            .all("div > p", Save::default())
            .then(|p| {
                [
                    p.all(
                        "span",
                        Save {
                            inner_html: true,
                            text_content: false,
                        },
                    ),
                    p.first(
                        "figure",
                        Save {
                            inner_html: false,
                            text_content: true,
                        },
                    )
                    .then(|figure| {
                        [
                            figure.first("> img", Save::default()),
                            figure.first("> figcaption", Save::default()).all(
                                "a",
                                Save {
                                    inner_html: true,
                                    text_content: true,
                                },
                            ),
                        ]
                    }),
                ]
            })
            .build();

        assert_eq!(query.list[0].source, "section");
        assert_eq!(query.list[0].parent, None);

        assert_eq!(query.list[1].source, "div > p");
        assert_eq!(query.list[1].parent, Some(0));

        assert_eq!(query.list[2].source, "span");
        assert_eq!(query.list[2].parent, Some(1));

        assert_eq!(query.list[3].source, "figure");
        assert_eq!(query.list[3].parent, Some(1));

        assert_eq!(query.list[4].source, "> img");
        assert_eq!(query.list[4].parent, Some(3));

        assert_eq!(query.list[5].source, "> figcaption");
        assert_eq!(query.list[5].parent, Some(3));

        assert_eq!(query.list[6].source, "a");
        assert_eq!(query.list[6].parent, Some(5));

        println!("{:#?}", query)
    }

    #[test]
    fn test_early_exit_basic() {
        let query = Query::first("section", Save::default()).build();
        assert_eq!(query.exit_at_section_end, Some(0));
    }

    #[test]
    fn test_failing_early_exit_basic() {
        let query = Query::all("section", Save::default()).build();
        assert_eq!(query.exit_at_section_end, None);
    }

    #[test]
    fn test_early_exit_big_tree() {
        let query = Query::first("section", Save::default())
            .first("div > p", Save::default())
            .then(|p| {
                [
                    p.first(
                        "span",
                        Save {
                            inner_html: true,
                            text_content: false,
                        },
                    ),
                    p.first(
                        "figure",
                        Save {
                            inner_html: false,
                            text_content: true,
                        },
                    )
                    .then(|figure| {
                        [
                            figure.first("> img", Save::default()),
                            figure.first("> figcaption", Save::default()).first(
                                "a",
                                Save {
                                    inner_html: true,
                                    text_content: true,
                                },
                            ),
                        ]
                    }),
                ]
            })
            .build();
        assert_eq!(query.exit_at_section_end, Some(1));
    }

    #[test]
    fn test_early_exit_big_list_deepest_exit() {
        let query = Query::first("section", Save::default()).build();
        assert_eq!(query.exit_at_section_end, Some(0));

        let query = Query::first("section", Save::default())
            .first("div > p", Save::default())
            .first("span", Save::none())
            .first("figure", Save::none())
            .first("> img", Save::default())
            .first("> figcaption", Save::default())
            .first("a", Save::none())
            .build();
        assert_eq!(query.exit_at_section_end, Some(6));
    }

    #[test]
    fn test_early_exit_big_tree_with_fully_valid_exits() {
        let query = Query::first("section", Save::default()).build();
        assert_eq!(query.exit_at_section_end, Some(0));

        let query = Query::first("section", Save::default())
            .first("div > p", Save::default())
            .then(|p| {
                [
                    p.first("span", Save::none()),
                    p.first("figure", Save::none()).then(|figure| {
                        [
                            figure.first("> img", Save::default()),
                            figure
                                .first("> figcaption", Save::default())
                                .first("a", Save::none()),
                        ]
                    }),
                ]
            })
            .build();
        assert_eq!(query.exit_at_section_end, Some(1));
    }
}
