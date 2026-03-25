use std::ops::Range;

use super::builder::{QueryBuilder, SelectionKind};
use super::transition::Transition;
use crate::Save;
use crate::css::element::Combinator;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position {
    pub(crate) selection: usize, // index in selection vec
    pub(crate) state: usize,     // index in state vec
}

impl<'query> Position {
    pub(crate) fn next_transition(&self, query: &Query<'query>) -> Option<usize> {
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

/// A single segment of a compiled [`Query`] tree.
///
/// Each `QuerySection` maps to one CSS selector string in the query chain
/// (e.g. `"main > section"` or `"> a[href]"`) and tracks its position in
/// the parent/sibling/child tree, what content to save, and whether it
/// matches all or only the first occurrence.
///
/// This type is internal bookkeeping; you rarely interact with it directly.
/// It is exposed publicly so that the [`Store`](crate::Store) can reference
/// query metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct QuerySection<'query> {
    /// The original CSS selector string for this section.
    pub(crate) source: &'query str,

    pub(super) range: std::ops::Range<usize>,

    pub(super) parent: Option<usize>,
    pub(super) next_sibling: Option<usize>,

    pub(crate) save: Save,
    pub(crate) kind: SelectionKind,
}

impl<'query> QuerySection<'query> {
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

/// A compiled CSS query ready to be executed against an HTML document.
///
/// A `Query` encapsulates a tree of [`QuerySection`]s, each representing
/// one CSS selector, compiled into an automaton of internal transitions.
/// The automaton is evaluated during streaming parsing to match elements
/// efficiently in a single pass.
///
/// # NFA Execution Model
///
/// Under the hood, a `Query` is compiled into a **Non-Deterministic Finite Automaton (NFA)**.
///
/// - **Fictitious States:** The NFA states themselves are implicit. They simply
///   represent the position (the integer index) between sequential transitions
///   within the automaton's evaluation path.
/// - **Transitions:** Defined by the internal `Transition` struct, each edge consists of a
///   `guard` (a topological `Combinator` dictating depth requirements like `>` or ` `)
///   and a `predicate` (an `ElementPredicate` matching tags, classes, etc.).
/// - **Branches:** A `QuerySection` represents a linear sequence of these transitions
///   (usually representing a single string selector). Branching your query with
///   [`QueryBuilder::then`] creates new sections that form a directed tree of sub-automata.
///
/// # Building a Query
///
/// Use [`Query::all`] or [`Query::first`] as entry points, then chain with
/// [`QueryBuilder::all`], [`QueryBuilder::first`], or [`QueryBuilder::then`],
/// and finalise with [`QueryBuilder::build`].
///
/// ```rust
/// use scah::{Query, Save};
///
/// // Simple: find all <a> tags
/// let q1 = Query::all("a", Save::all()).build();
///
/// // Compound: find sections, then extract links and text within them
/// let q2 = Query::all("section", Save::none())
///     .then(|s| [
///         s.all("a[href]", Save::all()),
///         s.first("p",     Save::only_text_content()),
///     ])
///     .build();
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct Query<'query> {
    pub(crate) states: Box<[Transition<'query>]>,
    pub(crate) queries: Box<[QuerySection<'query>]>,

    pub(crate) exit_at_section_end: Option<usize>,
}

impl<'query> Query<'query> {
    /// Start building a query that matches only the **first** element
    /// satisfying the given CSS selector.
    ///
    /// Using `first` enables an early-exit optimisation: once the
    /// match is found and its content captured, parsing of this branch
    /// can stop early.
    pub fn first(query: &'query str, save: Save) -> QueryBuilder<'query> {
        let states = Transition::generate_transitions_from_string(query);
        let queries = vec![QuerySection::new(
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

    /// Start building a query that matches **all** elements satisfying
    /// the given CSS selector.
    ///
    /// This is the most common entry point. The returned [`QueryBuilder`]
    /// can be chained with `.all()`, `.first()`, `.then()`, and finally
    /// `.build()` to produce a [`Query`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use scah::{Query, Save};
    ///
    /// let query = Query::all("a[href]", Save::all()).build();
    /// ```
    pub fn all(query: &'query str, save: Save) -> QueryBuilder<'query> {
        let states = Transition::generate_transitions_from_string(query);
        let queries = vec![QuerySection::new(
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

    pub(crate) fn get_transition(&self, state: usize) -> &Transition<'query> {
        &self.states[state]
    }

    pub(crate) fn get_section_selection_kind(&self, section_index: usize) -> &SelectionKind {
        &self.queries[section_index].kind
    }

    pub(crate) fn get_selection(&self, section_index: usize) -> &QuerySection<'query> {
        &self.queries[section_index]
    }

    pub(crate) fn is_descendant(&self, state: usize) -> bool {
        self.get_transition(state).guard == Combinator::Descendant
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

    pub(crate) fn can_move_foward(&self, position: &Position) -> bool {
        debug_assert!(position.selection < self.queries.len());

        let is_last_query = self.queries.len() - 1 == position.selection;
        let is_last_state = self.queries[position.selection].range.end - 1 == position.state;

        !is_last_query & is_last_state
    }
}

#[cfg(test)]
mod tests {
    use crate::css::element::AttributeSelection;
    use crate::css::element::AttributeSelectionKind;
    use crate::css::element::Combinator;
    use crate::css::element::ElementPredicate;
    use crate::css::selector::transition::Transition;
    use crate::{Query, QuerySection, Save, SelectionKind};

    #[test]
    fn test_query_builder_one_selection() {
        let query = Query::all("a", Save::all()).build();

        assert_eq!(
            query.states.iter().as_slice(),
            [Transition {
                predicate: ElementPredicate {
                    name: Some("a"),
                    id: None,
                    class: None,
                    attributes: vec![]
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
            .all("a", Save::all())
            .build();

        assert_eq!(
            query.states.iter().as_slice(),
            [
                Transition {
                    predicate: ElementPredicate {
                        name: Some("span"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                }
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                QuerySection {
                    source: "span",
                    save: Save::all(),
                    kind: SelectionKind::First,
                    parent: None,
                    range: 0..1,
                    next_sibling: None,
                },
                QuerySection {
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
                Transition {
                    predicate: ElementPredicate {
                        name: Some("div"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("span"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Child,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("p"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Child,
                },
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                QuerySection {
                    source: "div > span",
                    save: Save::all(),
                    kind: SelectionKind::First,
                    parent: None,
                    range: 0..2,
                    next_sibling: None,
                },
                QuerySection {
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
                Transition {
                    predicate: ElementPredicate {
                        name: Some("main"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("section"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Child,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![AttributeSelection {
                            name: "href",
                            value: None,
                            kind: AttributeSelectionKind::Presence
                        }]
                    },
                    guard: Combinator::Child,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("div"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
                Transition {
                    predicate: ElementPredicate {
                        name: Some("a"),
                        id: None,
                        class: None,
                        attributes: vec![]
                    },
                    guard: Combinator::Descendant,
                },
            ]
        );

        assert_eq!(
            query.queries.iter().as_slice(),
            [
                QuerySection {
                    source: "main > section",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: None,
                    range: 0..2,
                    next_sibling: None,
                },
                QuerySection {
                    source: "> a[href]",
                    save: Save::all(),
                    kind: SelectionKind::All,
                    parent: Some(0),
                    range: 2..3,
                    next_sibling: Some(2),
                },
                QuerySection {
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
