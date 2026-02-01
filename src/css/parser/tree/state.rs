use super::super::lexer::Lexer;
use crate::XHtmlElement;
use crate::css::parser::element::QueryElement;
use crate::css::parser::lexer::Combinator;
use crate::utils::Reader;

#[derive(PartialEq, Debug, Clone)]
pub struct State<'query> {
    pub transition: Combinator, // from transition
    pub state: QueryElement<'query>,
}

impl<'query> State<'query> {
    pub fn new(transition: Combinator, state: QueryElement<'query>) -> Self {
        Self { transition, state }
    }

    pub(super) fn generate_states_from_string(query: &'query str) -> Vec<Self> {
        let reader = &mut Reader::new(query);
        let mut states = Vec::new();
        while let Some((combinator, element)) = Lexer::next(reader) {
            states.push(Self::new(combinator, element));
        }

        states
    }

    pub fn next(
        &self,
        element: &XHtmlElement,
        current_depth: crate::css::query::DepthSize,
        last_depth: crate::css::query::DepthSize,
    ) -> bool {
        assert!(
            current_depth >= last_depth,
            "Current depth is smaller than last depth: {current_depth} >= {last_depth}"
        );

        if &self.state == element {
            return match self.transition {
                Combinator::Child => last_depth + 1 == current_depth,
                Combinator::Descendant => last_depth == 0 || current_depth != last_depth,

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
        element: &'html str,
        current_depth: crate::css::query::DepthSize,
        last_depth: crate::css::query::DepthSize,
    ) -> bool {
        if current_depth == last_depth {
            return self.state.name.is_some() && self.state.name.unwrap() == element;
        }
        return false;
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let state = State::new(
            Combinator::Descendant,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert!(state.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            },
            4,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_child() {
        let state = State::new(
            Combinator::Child,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert!(state.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            },
            2,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_child_failed() {
        let state = State::new(
            Combinator::Child,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert!(!state.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            },
            4,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_nextsibling() {
        let state = State::new(
            Combinator::NextSibling,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert!(state.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            },
            1,
            1,
        ));
    }
    #[test]
    fn test_fsm_next_subsequentsiblings() {
        let state = State::new(
            Combinator::SubsequentSibling,
            QueryElement {
                name: Some("a"),
                id: None,
                class: None,
                attributes: vec![],
            },
        );
        assert!(state.next(
            &XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: vec![]
            },
            1,
            1,
        ));
    }
}
