use super::manager::DocumentPosition;
use super::task::{FsmState, ScopedTask, Task};
//use super::tree::MatchTree;
use crate::XHtmlElement;
use crate::css::Save;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{Selection, SelectionKind};
//use crate::store::rust::Element;
use crate::store::{QueryError, Store};

struct SaveHook<E> {
    save: Save,
    on_depth: usize,
    element: *mut E,
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
    root: *mut E,
}

impl<'html, 'query: 'html, E> SelectionRunner<'query, E> {
    pub fn new(root: *mut E, selection_tree: &'query Selection<'query>) -> Self {
        Self {
            selection_tree,
            tasks: vec![Task::new(FsmState::new())],
            scoped_tasks: vec![],
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
                scoped_task.task.state.parent = store.push(
                    self.selection_tree
                        .get_section(scoped_task.task.state.position.section),
                    scoped_task.task.state.parent,
                    element.clone(),
                )?;
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
            println!("Match with `{:?}`", element);
            if self.selection_tree.get(&task.state.position).transition == Combinator::Descendant {
                // This should only be done if the task is not done (meaning it will move forward)
                self.scoped_tasks.push(ScopedTask::new(
                    document_position.element_depth,
                    task.state.parent,
                    task.state.position.clone(),
                ));
            }

            if self.selection_tree.is_save_point(&task.state.position) {
                task.state.parent = store.push(
                    self.selection_tree.get_section(task.state.position.section),
                    task.state.parent,
                    element.clone(),
                )?;
            }

            // TODO: move `move_foward` inside the task next
            // Selection should be able to know if their is a EndOfBranch or not
            let new_branch_tasks = task
                .state
                .move_foward(self.selection_tree, document_position.element_depth);
            if let Some(new_branch_tasks) = new_branch_tasks {
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
    ) {
        assert_ne!(self.tasks.len(), 0);

        let mut patterns_to_remove: Vec<usize> = vec![];
        // BUG: This should be in `back` not in `next`
        // if !scoped_task.in_scope(document_position.element_depth) {
        //     remove_scoped_at_index.push(i);
        //     continue;
        // }

        for i in 0..self.tasks.len() {
            let ref mut task = self.tasks[i];

            if task.state.back(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                let kind = self
                    .selection_tree
                    .get_section_selection_kind(task.state.position.section);
                if self.selection_tree.is_save_point(&task.state.position) {
                    // TODO: Add real Content
                    //self.tree.set_content(task.state.parent, 0, 0);
                }

                task.state.move_backward(self.selection_tree);
                /*if task.is_reset() {
                    // TODO: Remove the pattern
                    patterns_to_remove.push(i);
                    continue;
                }*/

                // TODO: Remove all retry point is it's equal to the current pattern
            }
        }

        //self.patterns.remove
    }
}
/*
mod tests {
    use super::super::tree::{ContentRange, Node};
    use crate::XHtmlElement;
    use crate::css::parser::element::QueryElement;
    use crate::css::parser::fsm::Fsm;
    use crate::css::parser::tree::{Position, Save, SelectionKind, SelectionPart};
    use crate::store::{Element, RustStore};
    use crate::utils::Reader;

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

        let mut selection = SelectionRunner::new(&mut store,&selection_tree);

        selection.next(
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

        assert_eq!(
            selection.tree.list,
            vec![Node {
                value: XHtmlElement {
                    name: "root",
                    class: None,
                    id: None,
                    attributes: Vec::new()
                },
                children: vec![],
                inner_html: ContentRange::Empty,
                text_content: ContentRange::Empty,
            },]
        );

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent: ..,
                    position: Position { section: 0, fsm: 1 },
                    depths: vec![0]
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
                        parent_tree_position: 0,
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![]
                    },
                },
            }]
        );

        selection.next(
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
            selection.tree.list,
            vec![
                Node {
                    value: XHtmlElement {
                        name: "root",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![1],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "a",
                        id: None,
                        class: None,
                        attributes: vec![],
                    },
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                    children: vec![],
                }
            ]
        );

        // assert_eq!(
        //     selection.tasks,
        //     vec![], // After First Selection, their is no other information to gather, thus the task is removed.
        // );

        assert_eq!(
            selection.scoped_tasks,
            vec![
                ScopedTask {
                    scope_depth: 0,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent_tree_position: 0,
                            position: Position { section: 0, fsm: 0 },
                            depths: vec![]
                        },
                    },
                },
                ScopedTask {
                    scope_depth: 1,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent_tree_position: 0,
                            position: Position { section: 0, fsm: 1 },
                            depths: vec![]
                        },
                    },
                }
            ]
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

        let mut selection = SelectionRunner::new(&selection_tree);

        selection.next(
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

        assert_eq!(
            selection.tree.list,
            vec![Node {
                value: XHtmlElement {
                    name: "root",
                    class: None,
                    id: None,
                    attributes: Vec::new()
                },
                children: vec![],
                inner_html: ContentRange::Empty,
                text_content: ContentRange::Empty,
            },]
        );

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent_tree_position: 0,
                    position: Position { section: 0, fsm: 1 },
                    depths: vec![0]
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
                        parent_tree_position: 0,
                        position: Position { section: 0, fsm: 0 },
                        depths: vec![]
                    },
                },
            }]
        );

        selection.next(
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
            selection.tree.list,
            vec![
                Node {
                    value: XHtmlElement {
                        name: "root",
                        class: None,
                        id: None,
                        attributes: Vec::new()
                    },
                    children: vec![1],
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                },
                Node {
                    value: XHtmlElement {
                        name: "p",
                        id: None,
                        class: Some("class"),
                        attributes: vec![],
                    },
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                    children: vec![],
                }
            ]
        );

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent_tree_position: 1,
                    position: Position { section: 2, fsm: 0 },
                    depths: vec![0, 1]
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
                            parent_tree_position: 0,
                            position: Position { section: 0, fsm: 0 },
                            depths: vec![]
                        },
                    },
                },
                ScopedTask {
                    scope_depth: 1,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent_tree_position: 0,
                            position: Position { section: 0, fsm: 1 },
                            depths: vec![]
                        },
                    },
                },
                ScopedTask {
                    scope_depth: 1,
                    task: Task {
                        retry_from: None,
                        state: FsmState {
                            parent_tree_position: 1,
                            position: Position { section: 1, fsm: 0 },
                            depths: vec![]
                        }
                    }
                },
            ]
        );
    }
}
*/
