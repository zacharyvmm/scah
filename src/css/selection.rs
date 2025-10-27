use super::pattern::Pattern;
use super::state::{Fsm, SelectionKind};
use super::tree::Tree;
use crate::XHtmlElement;

struct Selection<'query, 'html> {
    fsms: &'query Vec<Fsm<'query>>,
    patterns: Vec<Pattern>,
    retry_points: Vec<Pattern>,
    tree: Tree<'html>,
}

impl<'query, 'html> Selection<'query, 'html> {
    fn new(fsms: &'query Vec<Fsm<'query>>) -> Self {
        Self {
            fsms,
            patterns: vec![Pattern::new()],
            retry_points: Vec::new(),
            tree: Tree::new(),
        }
    }

    fn next(&mut self, depth: usize, element: &XHtmlElement<'html>) {
        assert_ne!(self.patterns.len(), 0);

        for i in 0..self.patterns.len() {
            let ref mut pattern = self.patterns[i];

            if pattern.next(self.fsms, depth, element) {
                println!("next");
                let kind = &self.fsms[pattern.position].state_kind;

                if pattern.retry(self.fsms) {
                    println!("retry");
                    self.retry_points.push(pattern.clone());
                }

                if *kind == SelectionKind::None {
                    println!("none");
                    pattern.move_foward(depth);
                    continue;
                }

                pattern.parent_save_position = self
                    .tree
                    .push(pattern.parent_save_position, element.clone());

                pattern.move_foward(depth);
                if matches!(*kind, SelectionKind::All(..)) {
                    continue;
                }

                // Step 3: Remove the pattern (if no textContent/innerHtml is needed)
                // Go back to the last ALL selection in the fsms list
                // If their are none then the whole selection is done
            }
            println!("{pattern:?}")
        }
    }

    fn back(&mut self, depth: usize, element: &'html str) {
        assert_ne!(self.patterns.len(), 0);

        let mut patterns_to_remove: Vec<usize> = vec![];

        for i in 0..self.patterns.len() {
            let ref mut pattern = self.patterns[i];

            if pattern.back(self.fsms, depth, element) {
                if self.fsms[pattern.position].state_kind != SelectionKind::None {
                    // TODO: Add real Content
                    self.tree
                        .set_content(pattern.parent_save_position, None, None);
                }

                pattern.move_backward();
                if pattern.is_reset() {
                    // TODO: Remove the pattern
                    patterns_to_remove.push(i);
                    continue;
                }

                // TODO: Remove all retry point is it's equal to the current pattern
            }
        }

        //self.patterns.remove
    }
}
mod tests {
    use crate::XHtmlElement;
    use crate::css::parser::element::element::QueryElement;
    use crate::css::parser::query_tokenizer::Combinator;

    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let fsms = &vec![
            Fsm::new(
                Combinator::Descendant,
                QueryElement {
                    name: Some("div"),
                    id: None,
                    class: None,
                    attributes: vec![],
                },
                SelectionKind::None,
            ),
            Fsm::new(
                Combinator::Descendant,
                QueryElement {
                    name: Some("a"),
                    id: None,
                    class: None,
                    attributes: vec![],
                },
                SelectionKind::None,
            ),
        ];

        let mut selection = Selection::new(fsms);

        selection.next(
            0,
            &XHtmlElement {
                name: "div",
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert_eq!(
            selection.patterns,
            vec![Pattern {
                parent_save_position: 0,
                position: 1,
                depths: vec![0]
            }]
        );
        assert_eq!(
            selection.retry_points,
            vec![Pattern {
                parent_save_position: 0,
                position: 0,
                depths: vec![]
            }]
        );

        selection.next(
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert_eq!(
            selection.patterns,
            vec![Pattern {
                parent_save_position: 0,
                position: 2,
                depths: vec![0, 1]
            }]
        );
        assert_eq!(
            selection.retry_points,
            vec![
                Pattern {
                    parent_save_position: 0,
                    position: 0,
                    depths: vec![]
                },
                Pattern {
                    parent_save_position: 0,
                    position: 1,
                    depths: vec![0]
                }
            ]
        );
    }
}
