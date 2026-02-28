use std::ops::Range;

use super::builder::{QueryBuilder, SelectionKind};
use super::state::State;
use crate::Save;
use crate::css::element::Combinator;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position {
    pub(crate) selection: usize, // index in selection vec
    pub(crate) state: usize,     // index in state vec
}

impl<'query> Position {
    pub(crate) fn next_state(&self, query: &Query<'query>) -> Option<usize> {
        debug_assert!(self.selection < query.queries.len());
        debug_assert!(query.queries[self.selection].range.contains(&self.state));

        let selection_range = &query.queries[self.selection.clone()].range;
        if self.state + 1 < selection_range.end {
            Some(self.state + 1)
        } else {
            None
        }
    }

    pub(crate) fn next_child(&self, query: &Query<'query>) -> Option<Self> {
        debug_assert!(self.selection < query.queries.len());
        debug_assert!(query.queries[self.selection].range.contains(&self.state));

        if self.selection == query.queries.len() - 1 {
            return None;
        }

        let next_selection_index = self.selection + 1;
        let next_selection = &query.queries[next_selection_index];
        if next_selection.parent.is_some_and(|p| p == self.selection) {
            return Some(Self {
                selection: next_selection_index,
                state: query.queries[next_selection_index].range.start,
            });
        }

        return None;
    }

    pub(crate) fn next_sibling(&self, query: &Query<'query>) -> Option<Self> {
        debug_assert!(self.selection < query.queries.len());
        debug_assert!(query.queries[self.selection].range.contains(&self.state));

        if let Some(sibling) = query.queries[self.selection].next_sibling {
            Some(Self {
                selection: sibling,
                state: query.queries[sibling].range.start,
            })
        } else {
            None
        }
    }

    pub(crate) fn is_root(&self) -> bool {
        //query.queries[self.selection].parent.is_none()

        self.selection == 0 && self.state == 0
    }

    pub(crate) fn back(&mut self, query: &Query<'query>) {
        debug_assert!(self.selection < query.queries.len());
        debug_assert!(self.state < query.queries[self.selection].range.end);

        let selection = &query.queries[self.selection];
        if self.state > selection.range.start {
            self.state -= 1;
        } else if let Some(parent) = selection.parent {
            self.selection = parent;
            self.state = query.queries[self.selection].range.end - 1;
        }
        // else it's at (0,0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection<'query> {
    pub(crate) source: &'query str,

    pub(super) range: std::ops::Range<usize>,

    pub(super) parent: Option<usize>,
    pub(super) next_sibling: Option<usize>,

    pub(crate) save: Save,
    pub(crate) kind: SelectionKind,
}

impl<'query> Selection<'query> {
    pub(super) fn new(
        source: &'query str,
        save: Save,
        kind: SelectionKind,
        range: Range<usize>,
        parent: Option<usize>,
    ) -> Self {
        Self {
            source,
            save,
            kind,
            range,

            parent,
            next_sibling: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Query<'query> {
    pub(crate) states: Box<[State<'query>]>,
    pub(crate) queries: Box<[Selection<'query>]>,

    #[deprecated(note = "Should be `exit at state end`")]
    pub(crate) exit_at_section_end: Option<usize>,
}

impl<'query> Query<'query> {
    pub fn first(query: &'query str, save: Save) -> QueryBuilder<'query> {
        let states = State::generate_states_from_string(query);
        let queries = vec![Selection::new(
            query,
            save,
            SelectionKind::First,
            0..states.len(),
            None,
        )];

        QueryBuilder {
            states,
            selection: queries,
        }
    }

    pub fn all(query: &'query str, save: Save) -> QueryBuilder<'query> {
        let states = State::generate_states_from_string(query);
        let queries = vec![Selection::new(
            query,
            save,
            SelectionKind::All,
            0..states.len(),
            None,
        )];

        QueryBuilder {
            states,
            selection: queries,
        }
    }

    pub(crate) fn get_state(&self, state: usize) -> &State<'query> {
        &self.states[state]
    }

    pub(crate) fn get_section_selection_kind(&self, section_index: usize) -> &SelectionKind {
        &self.queries[section_index].kind
    }

    pub(crate) fn get_selection(&self, section_index: usize) -> &Selection<'query> {
        &self.queries[section_index]
    }

    pub(crate) fn is_descendant(&self, state: usize) -> bool {
        self.get_state(state).transition == Combinator::Descendant
    }

    pub(crate) fn is_save_point(&self, position: &Position) -> bool {
        debug_assert!(
            self.queries[position.selection]
                .range
                .contains(&position.state)
        );
        let is_last_state = self.queries[position.selection].range.end - 1 == position.state;

        is_last_state
    }

    pub(crate) fn is_last_save_point(&self, position: &Position) -> bool {
        debug_assert!(position.selection < self.queries.len());

        let is_last_query = self.queries.len() - 1 == position.selection;
        let is_last_state = self.queries[position.selection].range.end - 1 == position.state;

        is_last_query & is_last_state
    }
}

#[cfg(test)]
mod tests {
    use crate::css::element::Combinator;
    use crate::css::element::{AttributeSelection, AttributeSelectionKind, QueryElement};
    use crate::css::selector::state::State;
    use crate::{Query, Save, Selection, SelectionKind};

    #[test]
    fn test_query_builder_one_selection() {
        let query = Query::all("a", Save::all()).build();

        assert_eq!(
            query.states.iter().as_slice(),
            [State {
                state: QueryElement {
                    name: Some("a"),
                    id: None,
                    class: None,
                    attributes: vec![]
                },
                transition: Combinator::Descendant,
            }]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [Selection {
                source: "a",
                save: Save::all(),
                kind: SelectionKind::All,
                parent: None,
                range: 0..1,
                next_sibling: None,
            }]
        );
    }

    #[test]
    fn test_query_builder_chainned_selection() {
        let query = Query::first("span", Save::all())
            .all("a", Save::all())
            .build();

        assert_eq!(
            query.states.iter().as_slice(),
            [
                State {
                    state: QueryElement {
                        name: Some("span"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
                State {
                    state: QueryElement {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                }
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                Selection {
                    source: "span",
                    save: Save::all(),
                    kind: SelectionKind::First,
                    parent: None,
                    range: 0..1,
                    next_sibling: None,
                },
                Selection {
                    source: "a",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: Some(0),
                    range: 1..2,
                    next_sibling: None,
                }
            ]
        );
    }

    #[test]
    fn test_query_builder_chainned_multi_element_selection() {
        let query = Query::first("div > span", Save::all())
            .all("p > a", Save::all())
            .build();

        assert_eq!(
            query.states.iter().as_slice(),
            [
                State {
                    state: QueryElement {
                        name: Some("div"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
                State {
                    state: QueryElement {
                        name: Some("span"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Child,
                },
                State {
                    state: QueryElement {
                        name: Some("p"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
                State {
                    state: QueryElement {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Child,
                },
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                Selection {
                    source: "div > span",
                    save: Save::all(),
                    kind: SelectionKind::First,
                    parent: None,
                    range: 0..2,
                    next_sibling: None,
                },
                Selection {
                    source: "p > a",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: Some(0),
                    range: 2..4,
                    next_sibling: None,
                }
            ]
        );
    }

    #[test]
    fn test_query_builder_chainned_multi_element_selection_with_branching() {
        let query = Query::all("main > section", Save::all())
            .then(|section| {
                [
                    section.all("> a[href]", Save::all()),
                    section.all("div a", Save::all()),
                ]
            })
            .build();

        assert_eq!(
            query.states.iter().as_slice(),
            [
                State {
                    state: QueryElement {
                        name: Some("main"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
                State {
                    state: QueryElement {
                        name: Some("section"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Child,
                },
                State {
                    state: QueryElement {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![AttributeSelection {
                            name: "href",
                            value: None,
                            kind: AttributeSelectionKind::Presence
                        }]
                    },
                    transition: Combinator::Child,
                },
                State {
                    state: QueryElement {
                        name: Some("div"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
                State {
                    state: QueryElement {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    transition: Combinator::Descendant,
                },
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                Selection {
                    source: "main > section",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: None,
                    range: 0..2,
                    next_sibling: None,
                },
                Selection {
                    source: "> a[href]",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: Some(0),
                    range: 2..3,
                    next_sibling: Some(2),
                },
                Selection {
                    source: "div a",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: Some(0),
                    range: 3..5,
                    next_sibling: None,
                }
            ]
        );
    }
}
