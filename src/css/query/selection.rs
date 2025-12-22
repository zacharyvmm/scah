use std::ops::Range;
use std::vec;

use super::manager::DocumentPosition;
use super::task::{FsmState, ScopedTask, Task};
//use super::tree::MatchTree;
use crate::XHtmlElement;
use crate::css::Save;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{NextPosition, Position, Selection};
//use crate::store::rust::Element;
use crate::store::{QueryError, Store};
use crate::utils::Reader;
use crate::xhtml::text_content::TextContent;

#[derive(Debug, Clone)]
enum ContentRange {
    None,
    Start(usize),
}
impl ContentRange {
    pub fn set_end<'html>(self, end: usize) -> Option<Range<usize>> {
        match self {
            Self::None => None,
            Self::Start(start) => Some(start..end),
        }
    }
}

#[derive(Debug)]
struct EndTagSaveContent<E> {
    element: *mut E,
    on_depth: usize,
    inner_html: ContentRange,
    text_content: ContentRange,
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
    tasks: Vec<Task<E>>,
    scoped_tasks: Vec<ScopedTask<E>>,
    on_close_tag_events: Vec<EndTagSaveContent<E>>,
    root: *mut E,
}

impl<'html, 'query: 'html, E> SelectionRunner<'query, E> {
    pub fn new(root: *mut E, selection_tree: &'query Selection<'query>) -> Self {
        Self {
            selection_tree,
            tasks: vec![Task::new(FsmState::new())],
            scoped_tasks: vec![],
            on_close_tag_events: vec![],
            root: root,
        }
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
        assert_ne!(self.tasks.len(), 0);

        // STEP 1: check scoped tasks
        let mut new_scoped_tasks: Vec<ScopedTask<E>> = vec![];

        for i in 0..self.scoped_tasks.len() {
            println!("Scoped Task {i}");
            let ref mut scoped_task = self.scoped_tasks[i];

            if !scoped_task.task.state.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                continue;
            }

            println!("Scope Match with `{:?}`", element);

            if self
                .selection_tree
                .get(&scoped_task.task.state.position)
                .transition
                == Combinator::Descendant
            {
                // This should only be done if the task is not done (meaning it will move forward)
                new_scoped_tasks.push(ScopedTask::new(
                    document_position.element_depth,
                    scoped_task.task.state.parent,
                    scoped_task.task.state.position.clone(),
                ));
            }

            let mut new_scoped_task = scoped_task.clone();
            let new_task = &mut new_scoped_task.task;
            if self
                .selection_tree
                .is_save_point(&scoped_task.task.state.position)
            {
                let section = self
                    .selection_tree
                    .get_section(scoped_task.task.state.position.section);

                let element_pointer =
                    store.push(section, scoped_task.task.state.parent, element.clone())?;
                if !self
                    .selection_tree
                    .is_last_save_point(&scoped_task.task.state.position)
                {
                    scoped_task.task.state.parent = element_pointer;
                }

                let Save {
                    inner_html,
                    text_content,
                } = section.kind.save();

                self.on_close_tag_events.push(EndTagSaveContent {
                    element: element_pointer,
                    on_depth: document_position.element_depth,
                    inner_html: if *inner_html {
                        ContentRange::Start(document_position.reader_position)
                    } else {
                        ContentRange::None
                    },
                    text_content: if *text_content {
                        ContentRange::Start(document_position.text_content_position)
                    } else {
                        ContentRange::None
                    },
                });
            }

            // TODO: move `move_foward` inside the task next
            // Selection should be able to know if their is a EndOfBranch or not
            let new_branch_tasks = new_task
                .state
                .move_foward(self.selection_tree, document_position.element_depth);
            if let Some(new_branch_tasks) = new_branch_tasks {
                new_scoped_tasks.append(
                    &mut new_branch_tasks
                        .into_iter()
                        .map(|pos| {
                            ScopedTask::new(
                                document_position.element_depth,
                                new_task.state.parent,
                                pos,
                            )
                        })
                        .collect(),
                );
            }

            new_scoped_tasks.push(new_scoped_task);
            // if position is end of fsm SelectionTree and all Selection are First Selection, then
        }
        self.scoped_tasks.append(&mut new_scoped_tasks);

        // STEP 2: check tasks
        for i in 0..self.tasks.len() {
            println!("Task {i}");
            let ref mut task = self.tasks[i];

            if !task.state.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                // println!(
                //     "(depth: {}) NO Match  `{:?}`",
                //     document_position.element_depth, task
                // );
                continue;
            }
            task.state.end = false;
            println!("Match with `{:?}`", element);

            let is_descendant_combinator =
                self.selection_tree.get(&task.state.position).transition == Combinator::Descendant;
            let last_save_point = self.selection_tree.is_last_save_point(&task.state.position);

            if is_descendant_combinator && !last_save_point {
                // This should only be done if the task is not done (meaning it will move forward)
                self.scoped_tasks.push(ScopedTask::new(
                    document_position.element_depth,
                    task.state.parent,
                    task.state.position.clone(),
                ));
            }

            if self.selection_tree.is_save_point(&task.state.position) {
                task.state.end = true;
                let section = self.selection_tree.get_section(task.state.position.section);
                let element_pointer = store.push(section, task.state.parent, element.clone())?;
                if !last_save_point {
                    task.state.parent = element_pointer;
                }

                let Save {
                    inner_html,
                    text_content,
                } = section.kind.save();

                self.on_close_tag_events.push(EndTagSaveContent {
                    element: element_pointer,
                    on_depth: document_position.element_depth,
                    inner_html: if *inner_html {
                        ContentRange::Start(document_position.reader_position)
                    } else {
                        ContentRange::None
                    },
                    text_content: if *text_content {
                        ContentRange::Start(document_position.text_content_position)
                    } else {
                        ContentRange::None
                    },
                });
            }

            // TODO: move `move_foward` inside the task next
            // Selection should be able to know if their is a EndOfBranch or not
            let new_branch_tasks = task
                .state
                .move_foward(self.selection_tree, document_position.element_depth);
            if let Some(new_branch_tasks) = new_branch_tasks {
                task.state.end = false;
                self.scoped_tasks.append(
                    &mut new_branch_tasks
                        .into_iter()
                        .map(|pos| {
                            ScopedTask::new(document_position.element_depth, task.state.parent, pos)
                        })
                        .collect(),
                );
            }

            // if position is end of fsm SelectionTree and all Selection are First Selection, then
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
        assert_ne!(self.tasks.len(), 0);

        let mut patterns_to_remove: Vec<usize> = vec![];
        // BUG: This should be in `back` not in `next`
        // if !scoped_task.in_scope(document_position.element_depth) {
        //     remove_scoped_at_index.push(i);
        //     continue;
        // }

        for i in (0..self.on_close_tag_events.len()).rev() {
            let content_trigger = &self.on_close_tag_events[i];
            if content_trigger.on_depth == document_position.element_depth {
                println!("Closing tag save content for `{element}`");
                let inner_html = {
                    let inner_enum = content_trigger.inner_html.clone();
                    let inner_range = inner_enum.set_end(document_position.reader_position);
                    if let Some(range) = inner_range {
                        let slice = reader.slice(range);
                        Some(slice)
                    } else {
                        None
                    }
                };
                let text_content = {
                    let tc_enum = content_trigger.text_content.clone();
                    if let Some(range) = tc_enum.set_end(document_position.text_content_position) {
                        let slice = content.join(range);
                        Some(slice)
                    } else {
                        None
                    }
                };
                store.set_content(content_trigger.element, inner_html, text_content);
                self.on_close_tag_events.remove(i);
                // TODO: Need to convert this to string
                // let inner_html_range = content_trigger
                //     .inner_html
                //     .set_end(document_position.reader_position);
                // let text_content_range = content_trigger
                //     .text_content
                //     .set_end(document_position.text_content_position);
                //store.set_content(content_trigger.element, None, None);
            }
        }

        self.scoped_tasks
            .retain(|scoped_task| scoped_task.scope_depth < document_position.element_depth);

        for i in 0..self.tasks.len() {
            let ref mut task = self.tasks[i];

            if !task.state.back(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                if task.state.end {
                    task.state.end = false;
                    // jump backwards twice
                    // TODO: refactor this, this is super hacky
                    task.state.position = self.selection_tree.back(&task.state.position);
                    if !task.state.back(
                        self.selection_tree,
                        document_position.element_depth,
                        element,
                    ) {
                        match self.selection_tree.next(&task.state.position) {
                            NextPosition::EndOfBranch => {
                                task.state.position = Position { section: 0, fsm: 0 };
                            }
                            NextPosition::Link(pos) => {
                                task.state.position = pos;
                            }
                            NextPosition::Fork(pos_list) => {
                                assert_ne!(pos_list.len(), 0, "Fork with no positions");
                                task.state.position = pos_list[0].clone();
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
                .get_section_selection_kind(task.state.position.section);
            if self.selection_tree.is_save_point(&task.state.position) {
                // TODO: Add real Content
                //self.tree.set_content(task.state.parent, 0, 0);
            }

            // if self.selection_tree.is_last_save_point(&task.state.position) {
            //     // TODO: Add real Content
            //     //self.tree.set_content(task.state.parent, 0, 0);
            //     continue;
            // }

            if self.selection_tree.is_save_point(&task.state.position) && task.state.end {
                assert!(task.state.depths.len() > 0);
                task.state.depths.pop();
                //task.state.end = false;
                continue;
            }

            task.state.move_backward(self.selection_tree);
            /*if task.is_reset() {
                // TODO: Remove the pattern
                patterns_to_remove.push(i);
                continue;
            }*/

            // TODO: Remove all retry point is it's equal to the current pattern
        }

        //self.patterns.remove
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
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 1 },
                    depths: vec![0],
                    end: false,
                }
            }]
        );

        assert_eq!(
            selection.scoped_tasks,
            vec![ScopedTask {
                scope_depth: 0,
                task: Task {
                    retry_from: None,
                    state: FsmState {
                        parent: std::ptr::null_mut(),
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![],
                        end: false,
                    },
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
            selection.scoped_tasks,
            vec![ScopedTask {
                scope_depth: 0,
                task: Task {
                    retry_from: None,
                    state: FsmState {
                        parent: std::ptr::null_mut(),
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![],
                        end: false,
                    },
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
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 1 },
                    depths: vec![0],
                    end: false,
                }
            }]
        );

        assert_eq!(
            selection.scoped_tasks,
            vec![ScopedTask {
                scope_depth: 0,
                task: Task {
                    retry_from: None,
                    state: FsmState {
                        parent: std::ptr::null_mut(),
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![],
                        end: false,
                    },
                },
            }]
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
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: mut_prt_unchecked!(&store.root.children["div p.class"].list[0]),
                    position: Position { section: 2, fsm: 0 },
                    depths: vec![0, 1],
                    end: false,
                }
            }]
        );

        assert_eq!(
            selection.scoped_tasks,
            vec![
                ScopedTask {
                    scope_depth: 0,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent: std::ptr::null_mut(),
                            position: Position { section: 0, fsm: 0 },
                            depths: vec![],
                            end: false,
                        },
                    },
                },
                ScopedTask {
                    scope_depth: 1,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent: std::ptr::null_mut(),
                            position: Position { section: 0, fsm: 1 },
                            depths: vec![],
                            end: false,
                        },
                    },
                },
                ScopedTask {
                    scope_depth: 1,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent: mut_prt_unchecked!(&store.root.children["div p.class"].list[0]),
                            position: Position { section: 1, fsm: 0 },
                            depths: vec![],
                            end: false,
                        }
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
        println!("{:?}", selection.tasks);

        assert_eq!(selection.scoped_tasks, vec![]);

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![0],
                    end: true,
                },
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

        assert_eq!(selection.scoped_tasks, vec![]);

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: std::ptr::null_mut(),
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![],
                    end: true,
                },
            },]
        );
    }
}
