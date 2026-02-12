use crate::{Query, Selection, css::parser::tree::state::State};
use std::pin::Pin;

use super::builder::{Save, SelectionKind};

#[derive(Debug, PartialEq, Clone)]
struct QueryString<S: AsRef<str>> {
    source: S,
    save: Save,
    kind: SelectionKind,

    parent: Option<usize>,
    next_sibling: Option<usize>,
}

pub struct LazyQuery {} 

impl LazyQuery {
    pub fn all<S: AsRef<str>>(query: S, save: Save) -> LazyQueryBuilder<S> {
        LazyQueryBuilder { queries: vec![QueryString {
            source: query,
            save: save,
            kind: SelectionKind::All,

            parent: None,
            next_sibling: None,
        }] }
    }

    pub fn first<S: AsRef<str>>(query: S, save: Save) -> LazyQueryBuilder<S> {
        LazyQueryBuilder { queries: vec![QueryString {
            source: query,
            save: save,
            kind: SelectionKind::First,

            parent: None,
            next_sibling: None,
        }] }
    }
}

// Basicly it's used for owned Strings, but I'll make it work for `&str`
#[derive(Clone)]
pub struct LazyQueryBuilder<S: AsRef<str>> {
    queries: Vec<QueryString<S>>,
}

impl<S: AsRef<str>> LazyQueryBuilder<S> {
    pub fn all_mut(&mut self, query: S, save: Save) {
        let parent_index = self.queries.len() - 1;
        self.queries.push(QueryString{
            source: query,
            save: save,
            kind: SelectionKind::All,
            parent: Some(parent_index),
            next_sibling: None,
        });
    }
    pub fn first_mut(&mut self, query: S, save: Save) {
        let parent_index = self.queries.len() - 1;
        self.queries.push(QueryString{
            source: query,
            save: save,
            kind: SelectionKind::First,
            parent: Some(parent_index),
            next_sibling: None,
        });
    }

    pub fn all(mut self, query: S, save: Save) -> Self {
        self.all_mut(query, save);
        self
    }

    pub fn first(mut self, query: S, save: Save) -> Self {
        self.first_mut(query, save);
        self
    }


    pub fn append(&mut self, parent: usize, mut other: Self) {
        let selection_length = self.queries.len();

        let mut last_sibling: Option<usize> = {
            if parent + 1 == selection_length {
                None
            } else {
                let mut sibling_index = parent + 1;
                while self.queries[sibling_index].next_sibling.is_some() {
                    sibling_index = self.queries[sibling_index].next_sibling.unwrap();
                }

                Some(sibling_index)
            }
        };
        for index in 0..other.queries.len() {
            let query = &mut other.queries[index];

            if let Some(idx) = query.parent {
                query.parent = Some(idx + selection_length);
            } else {
                query.parent = Some(parent);

                let current_index = selection_length + index;
                last_sibling = match last_sibling {
                    Some(sibling) => {
                        if sibling < selection_length {
                            self.queries[sibling].next_sibling = Some(current_index);
                        } else {
                            other.queries[sibling - selection_length].next_sibling =
                                Some(current_index);
                        }
                        Some(current_index)
                    }
                    None => Some(current_index),
                };
            }
        }
        self.queries.append(&mut other.queries);
    }


    pub fn then_mut<F, I>(&mut self, func: F)
    where
        F: FnOnce(LazyQueryFactory) -> I,
        I: IntoIterator<Item = Self>,
    {
        let factory = LazyQueryFactory {};
        let children = func(factory);

        let current_index = self.queries.len() - 1;
        for child in children {
            self.append(current_index, child);
        }
    }

    pub fn then<F, I>(mut self, func: F) -> Self
    where
        F: FnOnce(LazyQueryFactory) -> I,
        I: IntoIterator<Item = Self>,
    {
        self.then_mut(func);
        self
    }

    pub fn len(&self) -> usize {
        self.queries.len()
    }

    pub unsafe fn to_query<'a>(self) -> (String, Query<'a>) {
        // I need to do this to unsafely get a slice from the String
        let string_tape_size = self.queries.iter().map(|q| q.source.as_ref().len()).sum();
        let mut string_tape = String::with_capacity(string_tape_size);

        let mut queries = Vec::with_capacity(self.queries.len());
        let mut states = Vec::with_capacity(self.queries.len() * 2);

        for query in self.queries {
            let source = {
                let start = string_tape.len();
                string_tape.push_str(query.source.as_ref());
                let end = string_tape.len();

                unsafe{
                    let pointer = string_tape.as_ptr();

                    let raw_slice: &[u8] = std::slice::from_raw_parts(pointer.add(start), end - start);
                    str::from_utf8_unchecked(raw_slice)
                }
            };

            let mut string_states = State::generate_states_from_string(source);
            let range = {
                let start = states.len();
                states.append(&mut string_states);
                let end = states.len();

                start..end
            };


            queries.push(Selection {
                source,
                range,

                save: query.save,
                kind: query.kind,

                parent: query.parent,
                next_sibling: query.next_sibling,
            });

        }
        (string_tape, Query { 
            states: states.into_boxed_slice(),
            queries: queries.into_boxed_slice(),
            exit_at_section_end: None
        })
    }
}

pub struct LazyQueryFactory {}
impl<'query> LazyQueryFactory {
    pub fn all<S: AsRef<str>>(&self, query: S, save: Save) -> LazyQueryBuilder<S> {
        LazyQuery::all(query, save)
    }

    pub fn first<S: AsRef<str>>(&self, query: S, save: Save) -> LazyQueryBuilder<S> {
        LazyQuery::first(query, save)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use super::State;
    use crate::{Query, Save, SelectionKind};
    use crate::css::parser::lexer::Combinator;
    use crate::css::parser::element::{QueryElement, AttributeSelection, AttributeSelectionKind};

    #[test]
    fn test_lazy_query_selector() {
        let q = LazyQuery::all("div", Save::all())
        .first("a", Save::all()).all("a", Save::none());

        assert_eq!(q.queries, vec![
            QueryString {
                source: "div",
                save: Save::all(),
                kind: SelectionKind::All,
                parent: None,
                next_sibling: None,
            },
            QueryString {
                source: "a",
                save: Save::all(),
                kind: SelectionKind::First,
                parent: Some(0),
                next_sibling: None,
            },
            QueryString {
                source: "a",
                save: Save::none(),
                kind: SelectionKind::All,
                parent: Some(1),
                next_sibling: None,
            }
        ]);
    }

    #[test]
    fn test_lazy_query_selector_string() {
        let q = LazyQuery::all(String::from("div"), Save::all())
        .first(String::from("a"), Save::all()).all(String::from("a"), Save::none());

        assert_eq!(q.queries, vec![
            QueryString {
                source: String::from("div"),
                save: Save::all(),
                kind: SelectionKind::All,
                parent: None,
                next_sibling: None,
            },
            QueryString {
                source: String::from("a"),
                save: Save::all(),
                kind: SelectionKind::First,
                parent: Some(0),
                next_sibling: None,
            },
            QueryString {
                source: String::from("a"),
                save: Save::none(),
                kind: SelectionKind::All,
                parent: Some(1),
                next_sibling: None,
            }
        ]);
    }

    #[test]
    fn test_lazy_query_selector_then() {
        let q = LazyQuery::all("div", Save::all())
        .first("a", Save::all()).all("a", Save::none()).then(|a| [
            a.all("span", Save::all()),
            a.first("section", Save::all()).all("figure", Save::none()),
        ]);

        assert_eq!(q.queries, vec![
            QueryString {
                source: "div",
                save: Save::all(),
                kind: SelectionKind::All,
                parent: None,
                next_sibling: None,
            },
            QueryString {
                source: "a",
                save: Save::all(),
                kind: SelectionKind::First,
                parent: Some(0),
                next_sibling: None,
            },
            QueryString {
                source: "a",
                save: Save::none(),
                kind: SelectionKind::All,
                parent: Some(1),
                next_sibling: None,
            },
            QueryString {
                source: "span",
                save: Save::all(),
                kind: SelectionKind::All,
                parent: Some(2),
                next_sibling: Some(4),
            },
            QueryString {
                source: "section",
                save: Save::all(),
                kind: SelectionKind::First,
                parent: Some(2),
                next_sibling: None,
            },
            QueryString {
                source: "figure",
                save: Save::none(),
                kind: SelectionKind::All,
                parent: Some(4),
                next_sibling: None,
            },
        ]);
    }

    #[test]
    fn test_unsafe_slicing() {
        let q = LazyQuery::all(String::from("div"), Save::all())
        .first(String::from("a"), Save::all()).all(String::from("a"), Save::none());

        let (tape, query) = unsafe { q.to_query() };

        assert_eq!(tape, String::from("divaa"));
        //std::mem::drop(tape);

        assert_eq!(query, Query {
            states: vec![State::new(Combinator::Descendant, QueryElement {
                name: Some("div"),
                id: None,
                class: None,
                attributes: vec![]
            }),State::new(Combinator::Descendant, QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![]
            }),State::new(Combinator::Descendant, QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![]
            }),].into_boxed_slice(),
            queries: vec![
                Selection::new(
                    "div",
                    Save::all(),
                    SelectionKind::All,
                    0..1,
                    None,
                ),
                Selection::new(
                    "a",
                    Save::all(),
                    SelectionKind::First,
                    1..2,
                    Some(0),
                ),
                Selection::new(
                    "a",
                    Save::none(),
                    SelectionKind::All,
                    2..3,
                    Some(1),
                ),
            ].into_boxed_slice(),
            exit_at_section_end: None,
        });
    }
}