use super::query::{Query, QuerySection};
use super::transition::Transition;

/// Controls which pieces of content to capture for matched elements.
///
/// When an element matches a CSS selector, scah can optionally capture its
/// **inner HTML** (the raw markup between the opening and closing tags) and/or
/// its **text content** (the concatenated, whitespace-trimmed text nodes).
///
/// Use the convenience constructors [`Save::all`], [`Save::none`],
/// [`Save::only_inner_html`], and [`Save::only_text_content`] to create
/// common configurations.
///
/// # Example
///
/// ```rust
/// use scah::Save;
///
/// // Capture everything
/// let save = Save::all();
/// assert!(save.inner_html);
/// assert!(save.text_content);
///
/// // Capture only text content (lighter weight)
/// let save = Save::only_text_content();
/// assert!(!save.inner_html);
/// assert!(save.text_content);
/// ```
#[derive(PartialEq, Debug, Default, Clone, Copy)]
pub struct Save {
    /// When `true`, the raw HTML between the element's opening and closing
    /// tags is stored as [`Element::inner_html`](crate::Element::inner_html).
    pub inner_html: bool,
    /// When `true`, the concatenated text content of the element is stored
    /// and retrievable via [`Element::text_content()`](crate::Element::text_content).
    pub text_content: bool,
}

impl Save {
    /// Capture only the raw inner HTML of matched elements.
    pub fn only_inner_html() -> Self {
        Self {
            inner_html: true,
            text_content: false,
        }
    }

    /// Capture only the text content of matched elements.
    pub fn only_text_content() -> Self {
        Self {
            inner_html: false,
            text_content: true,
        }
    }

    /// Capture both inner HTML and text content.
    pub fn all() -> Self {
        Self {
            inner_html: true,
            text_content: true,
        }
    }

    /// Capture neither inner HTML nor text content.
    ///
    /// The matched element's tag name, id, class, and attributes are still
    /// stored; only the heavier content extraction is skipped.
    pub fn none() -> Self {
        Self {
            inner_html: false,
            text_content: false,
        }
    }
}

/// Whether a query section should match **all** occurrences or only the
/// **first** one.
///
/// Using [`SelectionKind::First`] enables an early-exit optimisation:
/// once the first match is found (and its content captured), the parser
/// can skip the remaining document for that query branch.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SelectionKind {
    /// Match every occurrence of the selector.
    All,
    /// Match only the first occurrence, enabling early exit.
    First,
}

/// An in-progress query being assembled via a builder pattern.
///
/// You typically don't construct a `QueryBuilder` directly; instead,
/// start with [`Query::all`] or [`Query::first`] and then chain further
/// selectors with [`.all()`](QueryBuilder::all), [`.first()`](QueryBuilder::first),
/// or [`.then()`](QueryBuilder::then). Finalise with [`.build()`](QueryBuilder::build)
/// to produce a [`Query`].
///
/// # Example
///
/// ```rust
/// use scah::{Query, Save};
///
/// let query = Query::all("main > section", Save::all())
///     .then(|ctx| [
///         ctx.all("> a[href]", Save::all()),
///         ctx.all("div a",    Save::only_text_content()),
///     ])
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct QueryBuilder<'query> {
    /// Internal automaton transitions (compiled selector segments).
    pub states: Vec<Transition<'query>>,
    /// Internal ordered list of query sections.
    pub selection: Vec<QuerySection<'query>>,
}

impl<'query> QueryBuilder<'query> {
    /// Append a child selector that matches **all** occurrences.
    ///
    /// The new selector is scoped to elements that already matched
    /// the previous selector in the chain.
    pub fn all(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.states.is_empty());
        assert!(!self.selection.is_empty());

        let current_state_len = self.states.len();
        let states = &mut Transition::generate_transitions_from_string(query);

        let parent_index = self.selection.len() - 1;
        let range = (current_state_len)..(current_state_len + states.len());
        self.selection.push(QuerySection::new(
            query,
            save,
            SelectionKind::All,
            range,
            Some(parent_index),
        ));

        self.states.append(states);

        self
    }

    /// Append a child selector that matches only the **first** occurrence.
    ///
    /// Enables early-exit optimisation for this branch of the query tree.
    pub fn first(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.states.is_empty());
        assert!(!self.selection.is_empty());

        let current_state_len = self.states.len();
        let states = &mut Transition::generate_transitions_from_string(query);

        let parent_index = self.selection.len() - 1;
        let range = (current_state_len)..(current_state_len + states.len());
        self.selection.push(QuerySection::new(
            query,
            save,
            SelectionKind::First,
            range,
            Some(parent_index),
        ));

        self.states.append(states);

        self
    }

    pub fn append(&mut self, parent: usize, mut other: Self) {
        let state_length = self.states.len();
        let selection_length = self.selection.len();

        let mut last_sibling: Option<usize> = {
            if parent + 1 == self.selection.len() {
                None
            } else {
                let mut sibling_index = parent + 1;
                while self.selection[sibling_index].next_sibling.is_some() {
                    sibling_index = self.selection[sibling_index].next_sibling.unwrap();
                }

                Some(sibling_index)
            }
        };
        for index in 0..other.selection.len() {
            let query = &mut other.selection[index];
            query.range.start += state_length;
            query.range.end += state_length;

            if let Some(idx) = query.parent {
                query.parent = Some(idx + selection_length);
            } else {
                query.parent = Some(parent);

                let current_index = selection_length + index;
                last_sibling = match last_sibling {
                    Some(sibling) => {
                        if sibling < selection_length {
                            self.selection[sibling].next_sibling = Some(current_index);
                        } else {
                            other.selection[sibling - selection_length].next_sibling =
                                Some(current_index);
                        }
                        Some(current_index)
                    }
                    None => Some(current_index),
                };
            }
        }
        self.states.append(&mut other.states);
        self.selection.append(&mut other.selection);
    }

    /// Branch into multiple child queries using a closure.
    ///
    /// The closure receives a [`QueryFactory`] that can create new
    /// sub-queries. Each sub-query is scoped to run only within elements
    /// matched by the current (most recently added) selector.
    ///
    /// This is the key mechanism for **structured querying**; extracting
    /// hierarchical data relationships in a single streaming pass.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scah::{Query, Save};
    ///
    /// let query = Query::all("article", Save::none())
    ///     .then(|article| [
    ///         article.first("h1",      Save::only_text_content()),
    ///         article.all("a[href]",   Save::all()),
    ///     ])
    ///     .build();
    /// ```
    pub fn then<F, I>(mut self, func: F) -> Self
    where
        F: FnOnce(QueryFactory) -> I,
        I: IntoIterator<Item = Self>,
    {
        let factory = QueryFactory {};
        let children = func(factory);

        let current_index = self.selection.len() - 1;
        for child in children {
            self.append(current_index, child);
        }
        self
    }

    fn exit_at_section(&self) -> Option<usize> {
        // returns the position in the selection tree where it can early exit
        // TODO: I should add a required flag for QuerySections, so that the first selection is nulled
        //  -> Basicly you can't return the first section without a perticular section behind added
        //  -> If you come back to the section without saving the required section,
        //      then you delete the saved data and you start over.

        fn search_for_single_exit_section(index: usize, list: &Vec<QuerySection>) -> Option<usize> {
            // If you have a section with MULTIPLE children that can early exit,
            //   then this parent node will become the exit section
            if index >= list.len() {
                return None;
            }
            let section = &list[index];
            let stop_here = match &section.kind {
                //BUG: you can only early exit when the ALL of them have been found, thus the parent must be awaited for
                SelectionKind::All => return None,

                // This is it need's to find the </{element}> to get either inner_html or text_content
                SelectionKind::First => section.save != Save::none(),
            };
            if stop_here {
                return Some(index);
            }

            let mut child = index + 1;
            if child >= list.len() {
                return Some(index);
            }

            let mut child_response: Option<usize> = None;
            if let Some(parent) = list[child].parent
                && parent == index
            {
                loop {
                    child_response = match child_response {
                        None => search_for_single_exit_section(child, list),
                        Some(_) => {
                            // If their's more than one child that can early exit then
                            // the parent is chosen
                            return Some(index);
                        }
                    };

                    if let Some(sibling) = list[child].next_sibling {
                        child = sibling;
                    } else {
                        break;
                    }
                }
            }

            if child_response.is_some() {
                return child_response;
            }
            Some(index)
        }

        search_for_single_exit_section(0, &self.selection)
    }
}

impl<'query> QueryBuilder<'query> {
    /// Finalise the builder and produce a compiled [`Query`].
    ///
    /// This computes early-exit optimisation metadata and converts the
    /// internal vectors into boxed slices. After calling `build`, pass
    /// the resulting `Query` to [`parse`](crate::parse).
    pub fn build(self) -> Query<'query> {
        let exit_at_section_end = self.exit_at_section();
        let states_box = self.states.into_boxed_slice();
        let query_box = self.selection.into_boxed_slice();
        Query {
            states: states_box,
            queries: query_box,
            exit_at_section_end,
        }
    }
}

/// A factory for creating child [`QueryBuilder`]s inside a
/// [`QueryBuilder::then`] closure.
///
/// You never construct this directly; it is provided as the argument to
/// the closure passed to `.then()`.
pub struct QueryFactory {}
impl<'query> QueryFactory {
    /// Create a child query that matches **all** occurrences of the selector.
    pub fn all(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        Query::all(query, save)
    }

    /// Create a child query that matches only the **first** occurrence.
    pub fn first(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        Query::first(query, save)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Query, Save};

    #[test]
    fn test_early_exit() {
        let query = Query::all("a", Save::all());
        assert_eq!(query.exit_at_section(), None);

        let query = Query::all("a", Save::none());
        assert_eq!(query.exit_at_section(), None);

        let query = Query::first("a", Save::all());
        assert_eq!(query.exit_at_section(), Some(0));

        let query = Query::first("a", Save::none());
        assert_eq!(query.exit_at_section(), Some(0));

        let query = Query::all("p", Save::all()).first("a", Save::all());
        assert_eq!(query.exit_at_section(), None);

        let query = Query::first("p", Save::all()).all("a", Save::all());
        assert_eq!(query.exit_at_section(), Some(0));

        let query = Query::first("p", Save::all()).first("a", Save::all());
        assert_eq!(query.exit_at_section(), Some(0));

        let query = Query::first("p", Save::none()).first("a", Save::none());
        assert_eq!(query.exit_at_section(), Some(1));
    }
}
