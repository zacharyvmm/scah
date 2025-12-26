use std::ops::Range;
use std::vec;

use super::manager::DocumentPosition;
use super::task::{FsmState, ScopedFsm};
//use super::tree::MatchTree;
use crate::XHtmlElement;
use crate::css::Save;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{NextPosition, Position, Selection};
//use crate::store::rust::Element;
use crate::store::{QueryError, Store};
use crate::utils::Reader;
use crate::xhtml::text_content::TextContent;

type StartIdx = Option<usize>;

#[derive(Debug)]
struct EndTagSaveContent<E> {
    element: *mut E,
    on_depth: usize,
    inner_html: StartIdx,
    text_content: StartIdx,
}

/*
 * A Selection works runs the fsm's using 2 types of tasks:
 * 1) the cursor tasks; this is a task that starts in the begining and always picks the last path.
 * 2) the scoped tasks; this is a task that is triggered by the cursor task of an other scoped task.
 *  The important distinction is that the scoped task terminates at a set scope depth (when <= to current depth: terminate).
 */

#[derive(Debug)]
pub struct SelectionRunner<'query, E> {
    selection_tree: &'query Selection<'query>,
    fsms: Vec<FsmState<E>>,
    scoped_fsms: Vec<ScopedFsm<E>>,
    on_close_tag_events: Vec<EndTagSaveContent<E>>,
    root: *mut E,
}

impl<'html, 'query: 'html, E> SelectionRunner<'query, E> {
    pub fn new(root: *mut E, selection_tree: &'query Selection<'query>) -> Self {
        Self {
            selection_tree,
            fsms: vec![FsmState::new()],
            scoped_fsms: vec![],
            on_close_tag_events: vec![],
            root: root,
        }
    }

    fn next_position(
        tree: &Selection<'query>,
        list: &mut Vec<ScopedFsm<E>>,
        depth: usize,
        fsm: &mut FsmState<E>,
    ) {
        let new_branch_tasks = fsm.move_foward(tree, depth);
        if let Some(new_branch_tasks) = new_branch_tasks {
            fsm.end = false;
            list.append(
                &mut new_branch_tasks
                    .into_iter()
                    .map(|pos| ScopedFsm::new(depth, fsm.parent, pos))
                    .collect(),
            );
        }
    }

    fn save_element<S>(
        on_close_tag_events: &mut Vec<EndTagSaveContent<E>>,
        tree: &Selection<'query>,
        store: &mut S,
        element: XHtmlElement<'html>,
        &DocumentPosition {
            element_depth,
            reader_position,
            text_content_position,
        }: &DocumentPosition,
        fsm: &mut FsmState<E>,
    ) -> Result<(), QueryError<'query>>
    where
        S: Store<'html, 'query, E = E>,
    {
        debug_assert!(fsm.is_save_point(tree));

        let section = tree.get_section(fsm.position.section);

        let element_pointer = store.push(section, fsm.parent, element)?;
        if !fsm.is_last_save_point(tree) {
            fsm.parent = element_pointer;
        }

        let Save {
            inner_html,
            text_content,
        } = section.kind.save();

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

    pub fn next<S>(
        &mut self,
        store: &mut S,
        element: &XHtmlElement<'html>,
        document_position: &DocumentPosition,
    ) -> Result<(), QueryError<'_>>
    where
        S: Store<'html, 'query, E = E>,
    {
        assert_ne!(self.fsms.len(), 0);

        // STEP 1: check scoped tasks
        let mut new_scoped_fsms: Vec<ScopedFsm<E>> = vec![];

        for i in 0..self.scoped_fsms.len() {
            println!("Scoped Fsm's {i}");
            let ref mut scoped_fsm = self.scoped_fsms[i];
            let fsm = &scoped_fsm.fsm;

            if !fsm.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                continue;
            }

            println!("Scope Match with `{:?}`", element);

            if fsm.is_descendant(self.selection_tree) {
                // This should only be done if the task is not done (meaning it will move forward)
                new_scoped_fsms.push(ScopedFsm::new(
                    document_position.element_depth,
                    fsm.parent,
                    fsm.position.clone(),
                ));
            }

            let mut new_scoped_fsm = scoped_fsm.clone();
            let new_fsm = &mut new_scoped_fsm.fsm;

            if new_fsm.is_save_point(self.selection_tree) {
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.selection_tree,
                    store,
                    element.clone(),
                    document_position,
                    new_fsm,
                )?;
            }

            Self::next_position(
                self.selection_tree,
                &mut new_scoped_fsms,
                document_position.element_depth,
                new_fsm,
            );

            new_scoped_fsms.push(new_scoped_fsm);
        }
        self.scoped_fsms.append(&mut new_scoped_fsms);

        // STEP 2: check tasks
        for i in 0..self.fsms.len() {
            println!("Fsm {i}");
            let ref mut fsm = self.fsms[i];

            if !fsm.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                continue;
            }
            println!("Match with `{:?}`", element);

            let is_descendant_combinator = fsm.is_descendant(self.selection_tree);
            let last_save_point = fsm.is_last_save_point(self.selection_tree);

            if is_descendant_combinator && !last_save_point {
                // This should only be done if the task is not done (meaning it will move forward)
                self.scoped_fsms.push(ScopedFsm::new(
                    document_position.element_depth,
                    fsm.parent,
                    fsm.position.clone(),
                ));
            }

            if fsm.is_save_point(self.selection_tree) {
                fsm.end = true;
                Self::save_element(
                    &mut self.on_close_tag_events,
                    self.selection_tree,
                    store,
                    element.clone(),
                    document_position,
                    fsm,
                )?;
            }

            Self::next_position(
                self.selection_tree,
                &mut self.scoped_fsms,
                document_position.element_depth,
                fsm,
            );
        }

        return Ok(());
    }

    pub fn back<S>(
        &mut self,
        store: &mut S,
        element: &'html str,
        document_position: &DocumentPosition,
        reader: &Reader<'html>,
        content: &TextContent,
    ) where
        S: Store<'html, 'query, E = E>,
    {
        assert_ne!(self.fsms.len(), 0);

        for i in (0..self.on_close_tag_events.len()).rev() {
            let content_trigger = &self.on_close_tag_events[i];
            if content_trigger.on_depth == document_position.element_depth {
                println!("Closing tag save content for `{element}`");
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
                        if start_idx == content.get_position() {
                            // No new text content was added after the element opened
                            None
                        } else {
                            // to skip the text content before the element (When the start was just opened, thus thier was no text content yet)
                            let slice = content.join((start_idx + 1)..);
                            Some(slice)
                        }
                    } else {
                        None
                    }
                };
                store.set_content(content_trigger.element, inner_html, text_content);
                self.on_close_tag_events.remove(i);
            }
        }

        self.scoped_fsms
            .retain(|scoped_task| scoped_task.scope_depth < document_position.element_depth);

        for i in 0..self.fsms.len() {
            let ref mut task = self.fsms[i];

            if !task.back(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                if task.end {
                    task.end = false;
                    // jump backwards twice
                    // TODO: refactor this, this is super hacky
                    task.position = self.selection_tree.back(&task.position);
                    if !task.back(
                        self.selection_tree,
                        document_position.element_depth,
                        element,
                    ) {
                        match self.selection_tree.next(&task.position) {
                            NextPosition::EndOfBranch => {
                                task.position = Position { section: 0, fsm: 0 };
                            }
                            NextPosition::Link(pos) => {
                                task.position = pos;
                            }
                            NextPosition::Fork(pos_list) => {
                                assert_ne!(pos_list.len(), 0, "Fork with no positions");
                                task.position = pos_list[0].clone();
                            }
                        }
                        continue;
                    }
                } else {
                    continue;
                }
            }
            println!("Saved `{}`", element);

            let kind = self
                .selection_tree
                .get_section_selection_kind(task.position.section);
            if self.selection_tree.is_save_point(&task.position) {}

            if self.selection_tree.is_save_point(&task.position) && task.end {
                assert!(task.depths.len() > 0);
                task.depths.pop();
                continue;
            }

            task.move_backward(self.selection_tree);
        }
    }
}
mod tests {
    use std::collections::HashMap;

    use crate::css::parser::element::QueryElement;
    use crate::css::parser::fsm::Fsm;
    use crate::css::parser::tree::{Position, Save, SelectionKind, SelectionPart};
    use crate::store::{Element, RustStore, SelectionValue, ValueKind};
    use crate::utils::Reader;
    use crate::{XHtmlElement, mut_prt_unchecked};

    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let section = SelectionPart::new(
            "div a",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = Selection::new(section);

        let mut store = RustStore::new();

        let mut selection = SelectionRunner::new(store.root(), &selection_tree);

        let _ = selection.next(
            &mut store,
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
        );

        assert_eq!(store.root.children, HashMap::new());

        assert_eq!(
            selection.fsms,
            vec![FsmState {
                parent: std::ptr::null_mut(),
                position: Position { section: 0, fsm: 1 },
                depths: vec![0],
                end: false,
            }]
        );

        assert_eq!(
            selection.scoped_fsms,
            vec![ScopedFsm {
                scope_depth: 0,
                fsm: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![],
                    end: false,
                },
            }]
        );

        let _ = selection.next(
            &mut store,
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
        );

        assert_eq!(
            store.root.children,
            HashMap::from([(
                "div a",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![Element {
                        name: "a",
                        id: None,
                        class: None,
                        attributes: vec![],
                        inner_html: None,
                        text_content: None,
                        children: HashMap::new(),
                    },]
                }
            )])
        );

        // assert_eq!(
        //     selection.tasks,
        //     vec![], // After First Selection, their is no other information to gather, thus the task is removed.
        // );

        assert_eq!(
            selection.scoped_fsms,
            vec![ScopedFsm {
                scope_depth: 0,
                fsm: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![],
                    end: false,
                },
            },]
        );
    }

    #[test]
    fn test_complex_fsm_query() {
        let mut selection_tree = Selection::new(SelectionPart::new(
            "div p.class",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: false,
            }),
        ));

        selection_tree.append(Vec::from([
            SelectionPart::new(
                "span",
                SelectionKind::First(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
            SelectionPart::new(
                "a",
                SelectionKind::First(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
        ]));

        let mut store = RustStore::new();
        let mut selection = SelectionRunner::new(store.root(), &selection_tree);

        let _ = selection.next(
            &mut store,
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
        );

        assert_eq!(store.root.children, HashMap::new());

        assert_eq!(
            selection.fsms,
            vec![FsmState {
                parent: std::ptr::null_mut(),
                position: Position { section: 0, fsm: 1 },
                depths: vec![0],
                end: false,
            }]
        );

        assert_eq!(
            selection.scoped_fsms,
            vec![ScopedFsm {
                scope_depth: 0,
                fsm: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![],
                    end: false,
                },
            },]
        );

        let _ = selection.next(
            &mut store,
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
        );

        assert_eq!(
            store.root.children,
            HashMap::from([(
                "div p.class",
                SelectionValue {
                    kind: ValueKind::SingleItem,
                    list: vec![Element {
                        name: "p",
                        id: None,
                        class: Some("class"),
                        attributes: vec![],
                        inner_html: None,
                        text_content: None,
                        children: HashMap::new(),
                    },]
                }
            )])
        );

        assert_eq!(
            selection.fsms,
            vec![FsmState {
                parent: mut_prt_unchecked!(&store.root.children["div p.class"].list[0]),
                position: Position { section: 2, fsm: 0 },
                depths: vec![0, 1],
                end: false,
            }]
        );

        assert_eq!(
            selection.scoped_fsms,
            vec![
                // ` div`
                ScopedFsm {
                    scope_depth: 0,
                    fsm: FsmState {
                        parent: std::ptr::null_mut(),
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![],
                        end: false,
                    },
                },
                // ` p.class`
                ScopedFsm {
                    scope_depth: 1,
                    fsm: FsmState {
                        parent: std::ptr::null_mut(),
                        position: Position { section: 0, fsm: 1 },
                        depths: vec![],
                        end: false,
                    },
                },
                // `> span`
                ScopedFsm {
                    scope_depth: 1,
                    fsm: FsmState {
                        parent: mut_prt_unchecked!(&store.root.children["div p.class"].list[0]),
                        position: Position { section: 1, fsm: 0 },
                        depths: vec![],
                        end: false,
                    }
                },
            ]
        );
    }

    #[test]
    fn test_simple_open_close() {
        let mut selection_tree = Selection::new(SelectionPart::new(
            "div",
            SelectionKind::First(Save {
                inner_html: false,
                text_content: false,
            }),
        ));

        let mut store = RustStore::new();
        let mut selection = SelectionRunner::new(store.root(), &selection_tree);

        let mut reader = Reader::new("<div></div>");
        let mut content = TextContent::new();

        let _ = selection.next(
            &mut store,
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
        );
        content.set_start(4);
        println!("{:?}", store);
        println!("{:?}", selection.fsms);

        assert_eq!(selection.scoped_fsms, vec![]);

        assert_eq!(
            selection.fsms,
            vec![FsmState {
                parent: std::ptr::null_mut(),
                position: Position { section: 0, fsm: 0 },
                depths: vec![0],
                end: true,
            },]
        );

        content.push(&reader, 4);
        let _ = selection.back(
            &mut store,
            "div",
            &DocumentPosition {
                reader_position: 0,
                text_content_position: 0,
                element_depth: 0,
            },
            &reader,
            &content,
        );

        assert_eq!(selection.scoped_fsms, vec![]);

        assert_eq!(
            selection.fsms,
            vec![FsmState {
                parent: std::ptr::null_mut(),
                position: Position { section: 0, fsm: 0 },
                depths: vec![],
                end: true,
            },]
        );
    }
}
