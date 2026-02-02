use super::query::{Query, Selection};
use super::state::State;

#[derive(PartialEq, Debug, Default, Clone, Copy)]
pub struct Save {
    // attributes: bool, // If your saving this has to be on
    pub inner_html: bool,
    pub text_content: bool,
}

impl Save {
    pub fn only_inner_html() -> Self {
        Self {
            inner_html: true,
            text_content: false,
        }
    }

    pub fn only_text_content() -> Self {
        Self {
            inner_html: false,
            text_content: true,
        }
    }

    pub fn all() -> Self {
        Self {
            inner_html: true,
            text_content: true,
        }
    }

    pub fn none() -> Self {
        Self {
            inner_html: false,
            text_content: false,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SelectionKind {
    All,
    First { locked: bool },
}

#[derive(Debug, Clone)]
pub struct QueryBuilder<'query> {
    pub states: Vec<State<'query>>,
    pub selection: Vec<Selection<'query>>,
}

impl<'query> QueryBuilder<'query> {
    pub fn all(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.states.is_empty());
        assert!(!self.selection.is_empty());

        let current_state_len = self.states.len();
        let states = &mut State::generate_states_from_string(query);

        let parent_index = self.selection.len() - 1;
        let range = (current_state_len)..(current_state_len + states.len());
        self.selection.push(Selection::new(
            query,
            save,
            SelectionKind::All,
            range,
            Some(parent_index),
        ));

        self.states.append(states);

        self
    }

    pub fn first(mut self, query: &'query str, save: Save) -> Self {
        assert!(!self.states.is_empty());
        assert!(!self.selection.is_empty());

        let current_state_len = self.states.len();
        let states = &mut State::generate_states_from_string(query);

        let parent_index = self.selection.len() - 1;
        let range = (current_state_len)..(current_state_len + states.len());
        self.selection.push(Selection::new(
            query,
            save,
            SelectionKind::First { locked: false },
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

    /*
    fn exit_at_section(&self) -> Option<usize> {
        // returns the position in the selection tree where it can early exit
        // TODO: I should add a required flag for QuerySections, so that the first selection is nulled
        //  -> Basicly you can't return the first section without a perticular section behind added
        //  -> If you come back to the section without saving the required section,
        //      then you delete the saved data and you start over.

        fn search_for_single_exit_section<S>(
            index: usize,
            list: &Vec<SelectionPart<S>>,
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
    */
}

impl<'query> QueryBuilder<'query> {
    pub fn build(self) -> Query<'query> {
        //let exit_at_section_end = self.exit_at_section();
        let states_box = self.states.into_boxed_slice();
        let query_box = self.selection.into_boxed_slice();
        Query {
            states: states_box,
            queries: query_box,
            exit_at_section_end: None,
        }
    }
}

pub struct QueryFactory {}
impl<'query> QueryFactory {
    pub fn all(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        Query::all(query, save)
    }

    pub fn first(&self, query: &'query str, save: Save) -> QueryBuilder<'query> {
        Query::first(query, save)
    }
}
