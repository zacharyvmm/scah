use std::ops::Range;

use super::builder::{QueryBuilder, Save, SelectionKind};
use super::error::SelectorParseError;
use super::transition::Transition;
use crate::query::selector::Combinator;

pub trait QuerySpec<'query> {
    fn states(&self) -> &[Transition<'query>];
    fn queries(&self) -> &[QuerySection<'query>];
    fn exit_at_section_end(&self) -> Option<usize>;

    fn get_transition(&self, state: usize) -> &Transition<'query> {
        &self.states()[state]
    }

    fn get_section_selection_kind(&self, section_index: usize) -> SelectionKind {
        self.queries()[section_index].kind
    }

    fn get_selection(&self, section_index: usize) -> &QuerySection<'query> {
        &self.queries()[section_index]
    }

    fn is_descendant(&self, state: usize) -> bool {
        self.get_transition(state).guard == Combinator::Descendant
    }

    fn is_save_point(&self, position: &Position) -> bool {
        debug_assert!(
            self.queries()[position.selection]
                .range
                .contains(&position.state)
        );
        self.queries()[position.selection].range.end - 1 == position.state
    }

    fn is_last_save_point(&self, position: &Position) -> bool {
        debug_assert!(position.selection < self.queries().len());
        let is_last_query = self.queries().len() - 1 == position.selection;
        let is_last_state = self.queries()[position.selection].range.end - 1 == position.state;
        is_last_query && is_last_state
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position {
    pub selection: usize,
    pub state: usize,
}

impl Position {
    pub fn next_transition<'query, Q: QuerySpec<'query> + ?Sized>(
        &self,
        query: &Q,
    ) -> Option<usize> {
        debug_assert!(self.selection < query.queries().len());
        debug_assert!(query.queries()[self.selection].range.contains(&self.state));

        let selection_range = &query.queries()[self.selection].range;
        if self.state + 1 < selection_range.end {
            Some(self.state + 1)
        } else {
            None
        }
    }

    pub fn next_child<'query, Q: QuerySpec<'query> + ?Sized>(&self, query: &Q) -> Option<Self> {
        debug_assert!(self.selection < query.queries().len());
        debug_assert!(query.queries()[self.selection].range.contains(&self.state));

        if self.selection == query.queries().len() - 1 {
            return None;
        }

        let next_selection_index = self.selection + 1;
        let next_selection = &query.queries()[next_selection_index];
        if next_selection.parent.is_some_and(|p| p == self.selection) {
            return Some(Self {
                selection: next_selection_index,
                state: query.queries()[next_selection_index].range.start,
            });
        }

        None
    }

    pub fn next_sibling<'query, Q: QuerySpec<'query> + ?Sized>(&self, query: &Q) -> Option<Self> {
        debug_assert!(self.selection < query.queries().len());
        debug_assert!(query.queries()[self.selection].range.contains(&self.state));

        query.queries()[self.selection]
            .next_sibling
            .map(|sibling| Self {
                selection: sibling,
                state: query.queries()[sibling].range.start,
            })
    }

    pub fn back<'query, Q: QuerySpec<'query> + ?Sized>(&mut self, query: &Q) {
        debug_assert!(self.selection < query.queries().len());
        debug_assert!(self.state < query.queries()[self.selection].range.end);

        let selection = &query.queries()[self.selection];
        if self.state > selection.range.start {
            self.state -= 1;
        } else if let Some(parent) = selection.parent {
            self.selection = parent;
            self.state = query.queries()[self.selection].range.end - 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuerySection<'query> {
    pub source: &'query str,
    pub range: Range<usize>,
    pub parent: Option<usize>,
    pub next_sibling: Option<usize>,
    pub save: Save,
    pub kind: SelectionKind,
}

impl<'query> QuerySection<'query> {
    pub fn new(
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

    pub const fn new_const(
        source: &'query str,
        save: Save,
        kind: SelectionKind,
        range: Range<usize>,
        parent: Option<usize>,
        next_sibling: Option<usize>,
    ) -> Self {
        Self {
            source,
            save,
            kind,
            range,
            parent,
            next_sibling,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Query<'query> {
    pub states: Box<[Transition<'query>]>,
    pub queries: Box<[QuerySection<'query>]>,
    pub exit_at_section_end: Option<usize>,
}

impl<'query> QuerySpec<'query> for Query<'query> {
    fn states(&self) -> &[Transition<'query>] {
        &self.states
    }

    fn queries(&self) -> &[QuerySection<'query>] {
        &self.queries
    }

    fn exit_at_section_end(&self) -> Option<usize> {
        self.exit_at_section_end
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct StaticQuery<'query, const N_STATES: usize, const N_SECTIONS: usize> {
    pub states: [Transition<'query>; N_STATES],
    pub queries: [QuerySection<'query>; N_SECTIONS],
    pub exit_at_section_end: Option<usize>,
}

impl<'query, const N_STATES: usize, const N_SECTIONS: usize>
    StaticQuery<'query, N_STATES, N_SECTIONS>
{
    pub const fn new(
        states: [Transition<'query>; N_STATES],
        queries: [QuerySection<'query>; N_SECTIONS],
        exit_at_section_end: Option<usize>,
    ) -> Self {
        Self {
            states,
            queries,
            exit_at_section_end,
        }
    }
}

impl<'query, const N_STATES: usize, const N_SECTIONS: usize> QuerySpec<'query>
    for StaticQuery<'query, N_STATES, N_SECTIONS>
{
    fn states(&self) -> &[Transition<'query>] {
        &self.states
    }

    fn queries(&self) -> &[QuerySection<'query>] {
        &self.queries
    }

    fn exit_at_section_end(&self) -> Option<usize> {
        self.exit_at_section_end
    }
}

impl<'query> Query<'query> {
    pub fn first(
        query: &'query str,
        save: Save,
    ) -> Result<QueryBuilder<'query>, SelectorParseError> {
        let states = Transition::generate_transitions_from_string(query)?;
        let queries = vec![QuerySection::new(
            query,
            save,
            SelectionKind::First,
            0..states.len(),
            None,
        )];

        Ok(QueryBuilder {
            states,
            selection: queries,
        })
    }

    pub fn all(query: &'query str, save: Save) -> Result<QueryBuilder<'query>, SelectorParseError> {
        let states = Transition::generate_transitions_from_string(query)?;
        let queries = vec![QuerySection::new(
            query,
            save,
            SelectionKind::All,
            0..states.len(),
            None,
        )];

        Ok(QueryBuilder {
            states,
            selection: queries,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::query::compiler::transition::Transition;
    use crate::query::selector::AttributeSelection;
    use crate::query::selector::AttributeSelectionKind;
    use crate::query::selector::AttributeSelections;
    use crate::query::selector::Combinator;
    use crate::query::selector::ElementPredicate;
    use crate::{Query, QuerySection, Save, SelectionKind};

    #[test]
    fn test_query_builder_one_selection() {
        let query = Query::all("a", Save::all()).unwrap().build();

        assert_eq!(
            query.states.iter().as_slice(),
            [Transition {
                predicate: ElementPredicate {
                    name: Some("a"),
                    id: None,
                    class: None,
                    attributes: AttributeSelections::from_static(&[])
                },
                guard: Combinator::Descendant,
            }]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [QuerySection {
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
            .unwrap()
            .all("a", Save::all())
            .unwrap()
            .build();

        assert_eq!(
            query.states.iter().as_slice(),
            [
                Transition {
                    predicate: ElementPredicate {
                        name: Some("span"),
                        id: None,
                        class: None,
                        attributes: AttributeSelections::from_static(&[])
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: AttributeSelections::from_static(&[])
                    },
                    guard: Combinator::Descendant,
                }
            ]
        );
    }

    #[test]
    fn test_query_builder_chainned_multi_element_selection() {
        let query = Query::first("span#top.inner", Save::all())
            .unwrap()
            .all("a#link1.foo[href^=\"https\"]", Save::all())
            .unwrap()
            .build();

        assert_eq!(query.states.len(), 2);
        assert_eq!(query.queries.len(), 2);
        assert_eq!(
            query.states[1].predicate,
            ElementPredicate {
                name: Some("a"),
                id: Some("link1"),
                class: Some("foo"),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "href",
                    value: Some("https"),
                    kind: AttributeSelectionKind::Prefix,
                }]),
            }
        );
    }

    #[test]
    fn test_query_builder_chainned_multi_element_selection_with_branching() {
        let query = Query::first("div", Save::all())
            .unwrap()
            .then(|ctx| {
                Ok([
                    ctx.all("a", Save::all())?,
                    ctx.first("p.note", Save::none())?,
                ])
            })
            .unwrap()
            .build();

        assert_eq!(query.queries.len(), 3);
        assert_eq!(query.queries[1].next_sibling, Some(2));
        assert_eq!(query.queries[2].next_sibling, None);
    }
}
