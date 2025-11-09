use super::manager::DocumentPosition;
use super::task::{FsmState, ScopedTask, Task};
use super::tree::MatchTree;
use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{SelectionKind, SelectionTree};

/*
 * A Selection works runs the fsm's using 2 types of tasks:
 * 1) the cursor tasks; this is a task that starts in the begining and always picks the last path.
 * 2) the scoped tasks; this is a task that is triggered by the cursor task of an other scoped task.
 *  The important distinction is that the scoped task terminates at a set scope depth (when <= to current depth: terminate).
 */

pub struct Selection<'query, 'html> {
    selection_tree: &'query SelectionTree<'query>,
    tasks: Vec<Task>,
    scoped_tasks: Vec<ScopedTask>,
    tree: MatchTree<'html>,
}

impl<'query, 'html> Selection<'query, 'html> {
    pub fn new(selection_tree: &'query SelectionTree<'query>) -> Self {
        Self {
            selection_tree,
            tasks: vec![Task::new(FsmState::new())],
            scoped_tasks: vec![],
            tree: MatchTree::new(),
        }
    }

    pub fn next(&mut self, element: &XHtmlElement<'html>, document_position: &DocumentPosition) {
        assert_ne!(self.tasks.len(), 0);

        // STEP 1: check scoped tasks

        let mut remove_scoped_at_index: Vec<usize> = vec![];
        let mut new_scoped_tasks: Vec<ScopedTask> = vec![];

        for i in 0..self.scoped_tasks.len() {
            let ref mut scoped_task = self.scoped_tasks[i];
            if !scoped_task.in_scope(document_position.element_depth) {
                remove_scoped_at_index.push(i);
                continue;
            }

            let task = &scoped_task.task;

            if task.state.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                let mut new_scoped_task = scoped_task.clone();
                let new_task = &mut new_scoped_task.task;

                if self.selection_tree.get(&task.state.position).transition
                    == Combinator::Descendant
                {
                    // This should only be done if the task is not done (meaning it will move forward)
                    new_scoped_tasks.push(ScopedTask::new(
                        document_position.element_depth,
                        task.state.parent_tree_position,
                        task.state.position.clone(),
                    ));
                }

                new_task.state.parent_tree_position = self.tree.push(
                    new_task.state.parent_tree_position,
                    element.clone(),
                    None,
                    None,
                );

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
                                    task.state.parent_tree_position,
                                    pos,
                                )
                            })
                            .collect(),
                    );
                }

                new_scoped_tasks.push(new_scoped_task);
                // if position is end of fsm SelectionTree and all Selection are First Selection, then
            }
        }
        for remove_index in remove_scoped_at_index.iter() {
            self.scoped_tasks.remove(*remove_index);
        }
        self.scoped_tasks.append(&mut new_scoped_tasks);

        // STEP 2: check tasks

        for i in 0..self.tasks.len() {
            let ref mut task = self.tasks[i];

            if task.state.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                if self.selection_tree.get(&task.state.position).transition
                    == Combinator::Descendant
                {
                    // This should only be done if the task is not done (meaning it will move forward)
                    self.scoped_tasks.push(ScopedTask::new(
                        document_position.element_depth,
                        task.state.parent_tree_position,
                        task.state.position.clone(),
                    ));
                }

                task.state.parent_tree_position =
                    self.tree
                        .push(task.state.parent_tree_position, element.clone(), None, None);

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
                                ScopedTask::new(
                                    document_position.element_depth,
                                    task.state.parent_tree_position,
                                    pos,
                                )
                            })
                            .collect(),
                    );
                }

                // if position is end of fsm SelectionTree and all Selection are First Selection, then
            }
        }
    }

    pub fn back(&mut self, element: &'html str, document_position: &DocumentPosition) {
        assert_ne!(self.tasks.len(), 0);

        let mut patterns_to_remove: Vec<usize> = vec![];

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
                    self.tree.set_content(task.state.parent_tree_position, 0, 0);
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

mod tests {
    use super::super::tree::{ContentRange, Node};
    use crate::XHtmlElement;
    use crate::css::parser::element::QueryElement;
    use crate::css::parser::fsm::Fsm;
    use crate::css::parser::tree::{Position, Save, SelectionKind, SelectionPart};
    use crate::utils::Reader;

    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let mut reader = Reader::new("div a");

        let section = SelectionPart::new(
            &mut reader,
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = SelectionTree::new(Vec::from([section]));

        let mut selection = Selection::new(&selection_tree);

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
                        name: "div",
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

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent_tree_position: 1,
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
                        name: "div",
                        id: None,
                        class: None,
                        attributes: vec![],
                    },
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                    children: vec![2],
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
                            parent_tree_position: 1,
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
        let mut first = Reader::new("div p.class");
        let mut second = Reader::new("span");
        let mut second_alternate = Reader::new("a");

        let mut selection_tree = SelectionTree::new(Vec::from([SelectionPart::new(
            &mut first,
            SelectionKind::First(Save {
                inner_html: false,
                text_content: false,
            }),
        )]));

        selection_tree.append(Vec::from([
            SelectionPart::new(
                &mut second,
                SelectionKind::First(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
            SelectionPart::new(
                &mut second_alternate,
                SelectionKind::First(Save {
                    inner_html: false,
                    text_content: false,
                }),
            ),
        ]));

        let mut selection = Selection::new(&selection_tree);

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
                        name: "div",
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

        assert_eq!(
            selection.tasks,
            vec![Task {
                retry_from: None,
                state: FsmState {
                    parent_tree_position: 1,
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
                        name: "div",
                        id: None,
                        class: None,
                        attributes: vec![],
                    },
                    inner_html: ContentRange::Empty,
                    text_content: ContentRange::Empty,
                    children: vec![2],
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
                    parent_tree_position: 2,
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
                            parent_tree_position: 1,
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
                            parent_tree_position: 2,
                            position: Position { section: 1, fsm: 0 },
                            depths: vec![]
                        }
                    }
                },
            ]
        );
    }
}
