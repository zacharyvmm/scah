use super::manager::DocumentPosition;
use super::task::Task;
use super::tree::MatchTree;
use crate::XHtmlElement;
use crate::css::parser::lexer::Combinator;
use crate::css::parser::tree::{SelectionKind, SelectionTree};

pub struct Selection<'query, 'html> {
    selection_tree: &'query SelectionTree<'query>,
    tasks: Vec<Task>,
    retry_points: Vec<Task>,
    tree: MatchTree<'html>,
}

impl<'query, 'html> Selection<'query, 'html> {
    pub fn new(selection_tree: &'query SelectionTree<'query>) -> Self {
        Self {
            selection_tree,
            tasks: vec![Task::new()],
            retry_points: Vec::new(),
            tree: MatchTree::new(),
        }
    }

    pub fn next(&mut self, element: &XHtmlElement<'html>, document_position: &DocumentPosition) {
        assert_ne!(self.tasks.len(), 0);

        let mut new_tasks: Vec<Task> = vec![];

        for i in 0..self.tasks.len() {
            let ref mut task = self.tasks[i];

            if task.next(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                let kind = self
                    .selection_tree
                    .get_section_selection_kind(task.position.section);

                if self.selection_tree.get(&task.position).transition == Combinator::Descendant {
                    self.retry_points.push(task.clone());
                }

                // TODO: move `move_foward` inside the task next
                // Selection should be able to know if their is a EndOfBranch or not
                let new_branch_tasks =
                    task.move_foward(self.selection_tree, document_position.element_depth);
                if let Some(new_branch_tasks) = new_branch_tasks {
                    new_tasks.append(
                        &mut new_branch_tasks
                            .into_iter()
                            .map(|pos| Task {
                                parent_tree_position: task.parent_tree_position,
                                position: pos,
                                depths: task.depths.clone(),
                            })
                            .collect(),
                    );
                }

                task.parent_tree_position =
                    self.tree
                        .push(task.parent_tree_position, element.clone(), None, None);

                if matches!(kind, SelectionKind::All(..)) {
                    continue;
                }

                // Step 3: Remove the pattern (if no textContent/innerHtml is needed)
                // Go back to the last ALL selection in the fsms list
                // If their are none then the whole selection is done
            } /*else if depth == *task.depths.last().unwrap()
            && (self.selection_tree.get(&task.position).transition == Combinator::NextSibling
            || (self.selection_tree.get(&task.position).transition == Combinator::SubsequentSibling)
            && matches!(
            self.selection_tree.get(&task.position).state_kind,
            SelectionKind::First(..)
            ))
            {
            task.move_backward();
            assert!(!task.is_reset());
            }*/
        }

        if new_tasks.len() > 0 {
            self.tasks.append(&mut new_tasks);
        }
    }

    pub fn back(&mut self, element: &'html str, document_position: &DocumentPosition) {
        assert_ne!(self.tasks.len(), 0);

        let mut patterns_to_remove: Vec<usize> = vec![];

        for i in 0..self.tasks.len() {
            let ref mut task = self.tasks[i];

            if task.back(
                self.selection_tree,
                document_position.element_depth,
                element,
            ) {
                let kind = self
                    .selection_tree
                    .get_section_selection_kind(task.position.section);
                if self.selection_tree.is_save_point(&task.position) {
                    // TODO: Add real Content
                    self.tree.set_content(task.parent_tree_position, 0, 0);
                }

                task.move_backward(self.selection_tree);
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
                parent_tree_position: 1,
                position: Position { section: 0, fsm: 1 },
                depths: vec![0]
            }]
        );

        assert_eq!(
            selection.retry_points,
            vec![Task {
                parent_tree_position: 0,
                position: Position { section: 0, fsm: 0 },
                depths: vec![]
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

        assert_eq!(
            selection.tasks,
            vec![], // Should be empty as the selection is complete, thus the retry points are used for start up of the next ALL selection
        );

        assert_eq!(
            selection.retry_points,
            vec![
                Task {
                    parent_tree_position: 0,
                    position: Position { section: 0, fsm: 0 },
                    depths: vec![]
                },
                Task {
                    parent_tree_position: 1,
                    position: Position { section: 0, fsm: 1 },
                    depths: vec![0]
                }
            ]
        );
    }
}
