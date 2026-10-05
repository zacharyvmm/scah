use std::ops::Range;

use super::builder::{QueryBuilder, Save, SelectionKind};
use super::error::SelectorParseError;
use super::transition::Transition;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct TransitionId(pub usize);

impl TransitionId {
    #[inline(always)]
    pub fn index(self) -> usize {
        self.0
    }
}

impl From<usize> for TransitionId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct QuerySectionId(pub usize);

impl QuerySectionId {
    #[inline(always)]
    pub fn index(self) -> usize {
        self.0
    }
}

impl From<usize> for QuerySectionId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRequirements {
    pub raw_text: bool,
    pub text: bool,
}

impl TextRequirements {
    pub fn any(self) -> bool {
        self.raw_text || self.text
    }
}

/// A compiled query, as read by [`Program::compile`](crate::Program::compile).
///
/// Sections own contiguous ranges of transitions. Each selector-list
/// alternative of a section is one sub-range, its consecutive compound
/// selectors.
pub trait QuerySpec<'query> {
    fn states(&self) -> &[Transition<'query>];
    fn queries(&self) -> &[QuerySection<'query>];
    fn selection_ranges(&self, section: QuerySectionId) -> &[Range<TransitionId>];
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuerySection<'query> {
    pub source: &'query str,
    pub range: Range<TransitionId>,
    pub parent: Option<QuerySectionId>,
    pub next_sibling: Option<QuerySectionId>,
    pub save: Save,
    pub kind: SelectionKind,
}

impl<'query> QuerySection<'query> {
    pub fn new(
        source: &'query str,
        save: Save,
        kind: SelectionKind,
        range: Range<TransitionId>,
        parent: Option<QuerySectionId>,
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
        range: Range<TransitionId>,
        parent: Option<QuerySectionId>,
        next_sibling: Option<QuerySectionId>,
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
    pub alternatives: Box<[Box<[Range<TransitionId>]>]>,
}

impl<'query> Query<'query> {
    pub fn new(
        states: Box<[Transition<'query>]>,
        queries: Box<[QuerySection<'query>]>,
        alternatives: Box<[Box<[Range<TransitionId>]>]>,
    ) -> Self {
        Self {
            states,
            queries,
            alternatives,
        }
    }
}

impl<'query> QuerySpec<'query> for Query<'query> {
    fn states(&self) -> &[Transition<'query>] {
        &self.states
    }

    fn queries(&self) -> &[QuerySection<'query>] {
        &self.queries
    }

    fn selection_ranges(&self, section: QuerySectionId) -> &[Range<TransitionId>] {
        &self.alternatives[section.index()]
    }
}

/// A query built at compile time by the `query!` macro.
#[derive(Debug, PartialEq, Clone)]
pub struct StaticQuery<'query, const N_STATES: usize, const N_SECTIONS: usize> {
    pub states: [Transition<'query>; N_STATES],
    pub queries: [QuerySection<'query>; N_SECTIONS],
    pub alternatives: &'query [&'query [Range<TransitionId>]],
}

impl<'query, const N_STATES: usize, const N_SECTIONS: usize>
    StaticQuery<'query, N_STATES, N_SECTIONS>
{
    pub const fn new(
        states: [Transition<'query>; N_STATES],
        queries: [QuerySection<'query>; N_SECTIONS],
        alternatives: &'query [&'query [Range<TransitionId>]],
    ) -> Self {
        Self {
            states,
            queries,
            alternatives,
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

    fn selection_ranges(&self, section: QuerySectionId) -> &[Range<TransitionId>] {
        self.alternatives[section.index()]
    }
}

impl<'query> Query<'query> {
    pub fn first(
        query: &'query str,
        save: Save,
    ) -> Result<QueryBuilder<'query>, SelectorParseError> {
        Self::build_initial(query, save, SelectionKind::First, false)
    }

    pub(crate) fn scoped(
        query: &'query str,
        save: Save,
        kind: SelectionKind,
    ) -> Result<QueryBuilder<'query>, SelectorParseError> {
        Self::build_initial(query, save, kind, true)
    }

    fn build_initial(
        query: &'query str,
        save: Save,
        kind: SelectionKind,
        scoped: bool,
    ) -> Result<QueryBuilder<'query>, SelectorParseError> {
        let paths = if scoped {
            Transition::generate_scoped_transition_paths_from_string(query)?
        } else {
            Transition::generate_transition_paths_from_string(query)?
        };
        let mut states = Vec::new();
        let mut alternatives = Vec::new();
        for path in paths {
            let start = TransitionId(states.len());
            states.extend(path);
            alternatives.push(start..TransitionId(states.len()));
        }
        let range = alternatives.first().unwrap().start..alternatives.last().unwrap().end;
        let queries = vec![QuerySection::new(query, save, kind, range, None)];

        Ok(QueryBuilder {
            states,
            selection: queries,
            alternatives: vec![alternatives],
        })
    }

    pub fn all(query: &'query str, save: Save) -> Result<QueryBuilder<'query>, SelectorParseError> {
        Self::build_initial(query, save, SelectionKind::All, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::query::compiler::transition::Transition;
    use crate::query::selector::AttributeSelection;
    use crate::query::selector::AttributeSelectionKind;
    use crate::query::selector::AttributeSelections;
    use crate::query::selector::ClassSelections;
    use crate::query::selector::Combinator;
    use crate::query::selector::ElementPredicate;
    use crate::{Query, QuerySection, QuerySectionId, Save, SelectionKind, TransitionId};

    #[test]
    fn test_query_builder_one_selection() {
        let query = Query::all("a", Save::all()).unwrap().build();

        assert_eq!(
            query.states.iter().as_slice(),
            [Transition::new(
                Combinator::Descendant,
                ElementPredicate {
                    name: Some("a"),
                    id: None,
                    classes: ClassSelections::from_static(&[]),
                    attributes: AttributeSelections::from_static(&[]),
                    logical: crate::LogicalPredicates::from_static(&[]),
                    structural: crate::StructuralPredicates::from_static(&[]),
                }
            )]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [QuerySection {
                source: "a",
                save: Save::all(),
                kind: SelectionKind::All,
                parent: None,
                range: TransitionId(0)..TransitionId(1),
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
                Transition::new(
                    Combinator::Descendant,
                    ElementPredicate {
                        name: Some("span"),
                        id: None,
                        classes: ClassSelections::from_static(&[]),
                        attributes: AttributeSelections::from_static(&[]),
                        logical: crate::LogicalPredicates::from_static(&[]),
                        structural: crate::StructuralPredicates::from_static(&[]),
                    }
                ),
                Transition::new(
                    Combinator::Descendant,
                    ElementPredicate {
                        name: Some("a"),
                        id: None,
                        classes: ClassSelections::from_static(&[]),
                        attributes: AttributeSelections::from_static(&[]),
                        logical: crate::LogicalPredicates::from_static(&[]),
                        structural: crate::StructuralPredicates::from_static(&[]),
                    }
                )
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
            query.states[1].predicate(),
            &ElementPredicate {
                name: Some("a"),
                id: Some("link1"),
                classes: ClassSelections::from_static(&["foo"]),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "href",
                    value: Some("https"),
                    kind: AttributeSelectionKind::Prefix,
                    case_sensitivity: crate::AttributeCaseSensitivity::Default,
                }]),
                logical: crate::LogicalPredicates::from_static(&[]),
                structural: crate::StructuralPredicates::from_static(&[]),
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
        assert_eq!(query.queries[1].next_sibling, Some(QuerySectionId(2)));
        assert_eq!(query.queries[2].next_sibling, None);
    }
}
