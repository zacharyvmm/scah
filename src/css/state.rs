use crate::XHtmlElement;
use crate::css::parser::element::QueryElement;
use crate::css::parser::query_tokenizer::Combinator;

#[derive(PartialEq)]
pub struct Save {
    // attributes: bool, // If your saving this has to be on
    pub inner_html: bool,
    pub text_content: bool,
}

#[derive(PartialEq)]
pub enum SelectionKind {
    First(Save),
    All(Save),
    None, // No Save
}

pub struct Fsm<'query> {
    pub transition: Combinator, // from transition
    pub state: QueryElement<'query>,
    pub state_kind: SelectionKind,
}

impl<'query> Fsm<'query> {
    pub fn new(
        transition: Combinator,
        state: QueryElement<'query>,
        state_kind: SelectionKind,
    ) -> Self {
        if transition == Combinator::SubsequentSibling
            && matches!(state_kind, SelectionKind::First(..))
        {
            println!(
                "WARNING: a `~` (Subsequent Sibling Combinator) with a First selection is equivalent to a `+` (Next Sibling Combinator) with a First selection."
            );
        }

        return Self {
            transition,
            state,
            state_kind,
        };
    }

    pub fn next<'html>(
        &self,
        current_depth: usize,
        last_depth: usize,
        element: &'html XHtmlElement<'html>,
    ) -> bool {
        assert!(current_depth >= last_depth);

        if &self.state == element {
            return match self.transition {
                Combinator::Child => last_depth + 1 == current_depth,
                Combinator::Descendant => true,

                // BUG: I need to know if it's the element right after
                // TODO: After first Fail it goes back
                Combinator::NextSibling => last_depth == current_depth,

                // BUG: I need to know if it's found a match before, so I know if it's ON/OFF
                Combinator::SubsequentSibling => true,

                Combinator::Namespace => panic!("Why are you using Namespace Selector ???"),
            };
        }

        return false;
    }

    pub fn back<'html>(
        &self,
        current_depth: usize,
        last_depth: usize,
        element: &'html str,
    ) -> bool {
        if current_depth == last_depth {
            return self.state.name.is_some() && self.state.name.unwrap() == element;
        }

        return false;
    }

    pub fn retry(&self) -> bool {
        return self.transition == Combinator::Descendant
            || (self.transition == Combinator::SubsequentSibling
                && matches!(self.state_kind, SelectionKind::All(..)));
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let fsm = Fsm::new(
            Combinator::Descendant,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        assert!(fsm.next(
            4,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            }
        ));
    }

    #[test]
    fn test_fsm_next_child() {
        let fsm = Fsm::new(
            Combinator::Child,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        assert!(fsm.next(
            2,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            }
        ));
    }

    #[test]
    fn test_fsm_next_child_failed() {
        let fsm = Fsm::new(
            Combinator::Child,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        assert!(!fsm.next(
            4,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            }
        ));
    }

    #[test]
    fn test_fsm_next_nextsibling() {
        let fsm = Fsm::new(
            Combinator::NextSibling,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        assert!(fsm.next(
            1,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            }
        ));
    }
    #[test]
    fn test_fsm_next_subsequentsiblings() {
        let fsm = Fsm::new(
            Combinator::SubsequentSibling,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        assert!(fsm.next(
            1,
            1,
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            }
        ));
    }
}
