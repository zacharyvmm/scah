use std::fmt::Debug;

use super::manager::DocumentPosition;
use super::task::{FsmState, ScopedFsm};
//use super::tree::MatchTree;
use crate::css::Save;
use crate::css::parser::tree::Query;
use crate::css::query::task::Fsm;
use crate::{XHtmlElement, dbg_print};
//use crate::store::rust::Element;
use crate::store::{QueryError, Store};
use crate::utils::Reader;

type StartIdx = Option<usize>;

#[derive(Debug)]
pub(crate) struct EndTagSaveContent {
    element: usize,
    on_depth: super::DepthSize,
    inner_html: StartIdx,
    text_content: StartIdx,
}

/*
 * A Selection works runs the fsm's using 2 types of tasks:
 * 1) the cursor tasks; this is a task that starts in the begining and always picks the last path.
 * 2) the scoped tasks; this is a task that is triggered by the cursor task of an other scoped task.
 *  The important distinction is that the scoped task terminates at a set scope depth (when <= to current depth: terminate).
 */

type ScopedFsmVec = Vec<ScopedFsm>;
type EndTagEventVec = Vec<EndTagSaveContent>;

#[derive(Debug)]
pub struct SelectionRunner<'a, 'query> {
    pub(crate) selection_tree: &'a Query<'query>,
    pub(crate) fsm: FsmState,
    pub(crate) scoped_fsms: ScopedFsmVec,
    pub(crate) on_close_tag_events: EndTagEventVec,
}

impl<'a, 'html, 'query: 'html> SelectionRunner<'a, 'query> {
    pub fn new(selection_tree: &'a Query<'query>) -> Self {
        Self {
            selection_tree,
            fsm: FsmState::new(),
            scoped_fsms: Vec::new(),
            on_close_tag_events: Vec::new(),
        }
    }

    fn next_position(
        tree: &Query<'query>,
        list: &mut ScopedFsmVec,
        depth: super::DepthSize,
        fsm: &mut impl Fsm<'query, 'html>,
    ) {
        if let Some(next_state) = fsm.get_position().next_state(tree) {
            fsm.set_state(next_state);
            fsm.add_depth(depth);
        } else if let Some(child) = fsm.get_position().next_child(tree) {
            fsm.set_position(child);
            fsm.set_end_false();

            let mut position = fsm.get_position().next_sibling(tree);
            while let Some(sibling) = position {
                position = sibling.next_sibling(tree);
                list.push(ScopedFsm::new(depth, fsm.get_parent(), sibling));
            }
        }
    }

    pub fn save_element(
        on_close_tag_events: &mut EndTagEventVec,
        tree: &Query<'query>,
        store: &mut Store<'html, 'query>,
        element: XHtmlElement<'html>,
        &DocumentPosition {
            element_depth,
            reader_position,
            text_content_position,
        }: &DocumentPosition,
        fsm: &mut impl Fsm<'query, 'html>,
    ) -> Result<(), QueryError<'query>> {
        // I can't check for this anymore, since the save is not instant and the fsm position is moved afterwards
        //debug_assert!(fsm.is_save_point(tree));

        let section = tree.get_selection(fsm.get_position().selection);

        let element_pointer = store.push(section, fsm.get_parent(), element);
        if !tree.is_last_save_point(fsm.get_position()) {
            fsm.set_parent(element_pointer);
        }

        let Save {
            inner_html,
            text_content,
        } = &section.save;

        on_close_tag_events.push(EndTagSaveContent {
            element: element_pointer,
            on_depth: element_depth,
            inner_html: if *inner_html {
                // Since thiis is triggered on opening tag, the start is the current position in the content
                // array is about the previous elements text content item, thus I need to add 1 to get the correct position
                // Their could be a BUG here if there is no text content ("" -> no item added)
                Some(reader_position)
            } else {
                None
            },
            text_content: if *text_content {
                Some(text_content_position)
            } else {
                None
            },
        });

        Ok(())
    }

    pub fn next(
        &mut self,
        element: &XHtmlElement<'html>,
        document_position: &DocumentPosition,
        store: &mut Store<'html, 'query>,
    ) -> Result<(), QueryError<'_>> {
        // STEP 1: check scoped tasks
        let mut new_scoped_fsms: ScopedFsmVec = Vec::new();

        for i in 0..self.scoped_fsms.len() {
            // println!("Scoped Fsm's {i}");
            let fsm = &mut self.scoped_fsms[i];

            if !fsm.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                continue;
            }

            dbg_print!("Scoped FSM ({i}) Match with `{:?}`", element);

            // println!("Scope Match with `{:?}`", element);

            if self.selection_tree.is_descendant(fsm.get_position().state) {
                // This should only be done if the task is not done (meaning it will move forward)
                new_scoped_fsms.push(ScopedFsm::new(
                    document_position.element_depth,
                    fsm.parent,
                    fsm.position,
                ));
            }

            let mut new_scoped_fsm = fsm.clone();

            if self.selection_tree.is_save_point(&new_scoped_fsm.position) {
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.selection_tree,
                    store,
                    element.clone(),
                    document_position,
                    &mut new_scoped_fsm,
                )?;

                dbg_print!("Scoped FSM ({i}) Saved `{:?}`", element);
            }

            if !element.is_self_closing() {
                Self::next_position(
                    self.selection_tree,
                    &mut new_scoped_fsms,
                    document_position.element_depth,
                    &mut new_scoped_fsm,
                );
            }

            new_scoped_fsms.push(new_scoped_fsm);

            dbg_print!(">> Scoped FSM's: {:#?}", self.scoped_fsms)
        }
        self.scoped_fsms.append(&mut new_scoped_fsms);

        // STEP 2: check tasks
        let ref mut fsm = self.fsm;

        if fsm.next(
            self.selection_tree,
            document_position.element_depth,
            element,
        ) {
            dbg_print!("FSM Match with `{:?}`", element);

            let is_descendant_combinator = self.selection_tree.is_descendant(fsm.position.state);
            let last_save_point = self.selection_tree.is_last_save_point(&fsm.position);
            let section_kind = self
                .selection_tree
                .get_section_selection_kind(fsm.position.selection);
            let is_all = matches!(section_kind, crate::css::SelectionKind::All);
            let is_root = fsm.position.is_root();

            if is_descendant_combinator && !last_save_point {
                // This should only be done if the task is not done (meaning it will move forward)
                self.scoped_fsms.push(ScopedFsm::new(
                    document_position.element_depth,
                    fsm.parent,
                    fsm.position,
                ));
            } else if is_all && !is_root {
                let scope = fsm.depths.last().copied().unwrap_or(0);
                self.scoped_fsms
                    .push(ScopedFsm::new(scope, fsm.parent, fsm.position));
            }

            // let parent: *mut E = fsm.parent;

            if self.selection_tree.is_save_point(&fsm.position) {
                fsm.end = true;
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.selection_tree,
                    store,
                    element.clone(),
                    document_position,
                    fsm,
                )?;

                dbg_print!("FSM Saved `{:?}`", element);
            }

            if !element.is_self_closing() {
                // let new_parent = fsm.parent;
                // fsm.set_parent(parent);
                Self::next_position(
                    self.selection_tree,
                    &mut self.scoped_fsms,
                    document_position.element_depth,
                    fsm,
                );
                // fsm.set_parent(new_parent);
            }
            dbg_print!("Scoped FSM's: {:#?}", self.scoped_fsms)
        }

        return Ok(());
    }

    pub fn early_exit(&self) -> bool {
        if let Some(early_exit_section) = self.selection_tree.exit_at_section_end {
            return early_exit_section == self.fsm.position.selection;
        }

        false
    }

    pub fn back(
        &mut self,
        store: &mut Store<'html, 'query>,
        element: &'html str,
        document_position: &DocumentPosition,
        reader: &Reader<'html>,
    ) -> bool {
        for i in (0..self.on_close_tag_events.len()).rev() {
            let content_trigger = &self.on_close_tag_events[i];
            if content_trigger.on_depth == document_position.element_depth {
                // println!("Closing tag save content for `{element}`");
                let inner_html = {
                    if let Some(start_idx) = content_trigger.inner_html {
                        let slice = reader.slice(start_idx..document_position.reader_position);
                        Some(slice)
                    } else {
                        None
                    }
                };
                let text_content = {
                    if let Some(start_idx) = content_trigger.text_content {
                        if start_idx == usize::MAX {
                            if store.text_content.is_empty() {
                                None
                            } else {
                                Some(0..store.text_content.get_position())
                            }
                        } else if start_idx == store.text_content.get_position() {
                            // No new text content was added after the element opened
                            None
                        } else {
                            // to skip the text content before the element (When the start was just opened, thus thier was no text content yet)
                            Some((start_idx + 1)..store.text_content.get_position())
                        }
                    } else {
                        None
                    }
                };
                store.set_content(content_trigger.element, inner_html, text_content);
                self.on_close_tag_events.remove(i);
            }
        }

        // self.scoped_fsms
        //     .retain(|scoped_task| scoped_task.scope_depth < document_position.element_depth);

        let mut remove_last_x_fsms = 0;
        for scoped_fsm in self.scoped_fsms.iter().rev() {
            if scoped_fsm.scope_depth <= document_position.element_depth {
                self.fsm.parent = scoped_fsm.parent;
                break;
            }
            remove_last_x_fsms += 1;
        }
        while remove_last_x_fsms > 0 {
            self.scoped_fsms.pop();
            remove_last_x_fsms -= 1;
        }

        let ref mut fsm = self.fsm;
        dbg_print!("FSM Before back: {:#?}", fsm);
        if fsm.back(
            self.selection_tree,
            document_position.element_depth,
            element,
        ) {
            fsm.step_backward(self.selection_tree);
            dbg_print!("FSM out of `{}`", element);
            dbg_print!("SHOULD INVALIDATED PARENT POINTER");
            return true;
        } else if fsm.end {
            // jump backwards twice
            if fsm.try_back_parent(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                fsm.move_backward_twice(self.selection_tree);
                fsm.end = false;

                dbg_print!("FSM out of `{}`", element);
                dbg_print!("SHOULD INVALIDATED PARENT POINTER");
                return true;
            }
        }

        return false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::XHtmlElement;
    use crate::css::parser::tree::{Position, Query, Save};
    use crate::store::ChildIndex;
    use crate::store::Store;
    use crate::utils::Reader;
    use smallvec::smallvec;

    const NULL_PARENT: usize = 0;

    #[test]
    fn test_fsm_next_descendant() {
        let selection_tree = &Query::all("div a", Save::none()).build();

        let mut store = Store::new();

        let mut selection = SelectionRunner::new(selection_tree);

        let _ = selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );

        assert!(store.elements[0].children.is_empty());

        assert_eq!(
            selection.fsm,
            FsmState {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 1
                },
                depths: smallvec![0],
                end: false,
            }
        );

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![ScopedFsm {
                scope_depth: 0,
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
            }]
        );

        let _ = selection.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 1,
            },
            &mut store,
        );

        assert_eq!(store.elements[0].children.len(), 1);
        let child = &store.elements[0].children[0];
        assert_eq!(child.query, "div a");
        match &child.index {
            ChildIndex::Many(indices) => assert_eq!(indices, &vec![1]),
            _ => panic!("Expected Many"),
        }

        assert_eq!(store.elements[1].name, "a");

        // assert_eq!(
        //     selection.tasks,
        //     smallvec![], // After First Selection, their is no other information to gather, thus the task is removed.
        // );

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![
                ScopedFsm {
                    scope_depth: 0,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 0
                    },
                },
                ScopedFsm {
                    scope_depth: 0,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 1
                    },
                }
            ]
        );
    }

    #[test]
    fn test_complex_fsm_query() {
        let selection_tree = &Query::first("div p.class", Save::none())
            .then(|p| [p.first("span", Save::none()), p.first("a", Save::none())])
            .build();

        let mut store = Store::new();
        let mut selection = SelectionRunner::new(selection_tree);

        let _ = selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );

        assert!(store.elements[0].children.is_empty());

        assert_eq!(
            selection.fsm,
            FsmState {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 1
                },
                depths: smallvec![0],
                end: false,
            }
        );

        assert_eq!(selection.scoped_fsms.len(), 1);
        assert_eq!(
            selection.scoped_fsms[0],
            ScopedFsm {
                scope_depth: 0,
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
            }
        );

        let _ = selection.next(
            &XHtmlElement {
                name: "p",
                id: None,
                class: Some("class"),
                attributes: vec![],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 1,
            },
            &mut store,
        );

        assert_eq!(store.elements[0].children.len(), 1);
        let child = &store.elements[0].children[0];
        assert_eq!(child.query, "div p.class");
        match &child.index {
            ChildIndex::One(idx) => assert_eq!(*idx, 1),
            _ => panic!("Expected One"),
        }
        assert_eq!(store.elements[1].name, "p");

        assert_eq!(
            selection.fsm,
            FsmState {
                parent: 1,
                position: Position {
                    selection: 2,
                    state: 3
                },
                depths: smallvec![0, 1],
                end: false,
            }
        );

        assert_eq!(
            selection.scoped_fsms.to_vec(),
            vec![
                // ` div`
                ScopedFsm {
                    scope_depth: 0,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 0
                    },
                },
                // ` p.class`
                ScopedFsm {
                    scope_depth: 1,
                    parent: NULL_PARENT,
                    position: Position {
                        selection: 0,
                        state: 1
                    },
                },
                // `> span`
                ScopedFsm {
                    scope_depth: 1,
                    parent: 1,
                    position: Position {
                        selection: 1,
                        state: 2
                    },
                },
            ]
        );
    }

    #[test]
    fn test_simple_open_close() {
        let selection_tree = Query::first("div", Save::none()).build();

        let mut store = Store::new();
        let mut selection = SelectionRunner::new(&selection_tree);

        let reader = Reader::new("<div></div>");

        let _ = selection.next(
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
            },
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &mut store,
        );
        store.text_content.set_start(4);
        println!("{:?}", store);
        println!("{:?}", selection.fsm);

        assert!(selection.scoped_fsms.is_empty());

        assert_eq!(
            selection.fsm,
            FsmState {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
                depths: smallvec![0],
                end: true,
            }
        );

        store.text_content.push(&reader, 4);
        let _ = selection.back(
            &mut store,
            "div",
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &reader,
        );

        assert!(selection.scoped_fsms.is_empty());

        assert_eq!(
            selection.fsm,
            FsmState {
                parent: NULL_PARENT,
                position: Position {
                    selection: 0,
                    state: 0
                },
                depths: smallvec![],
                end: true,
            }
        );
    }
}
