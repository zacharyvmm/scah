use crate::XHtmlElement;
use crate::css::element::{Combinator, ElementPredicate, Lexer};
use crate::utils::Reader;

#[derive(PartialEq, Debug, Clone)]
pub struct Transition<'query> {
    pub guard: Combinator, // from transition
    pub predicate: ElementPredicate<'query>,
}

impl<'query> Transition<'query> {
    pub fn new(guard: Combinator, predicate: ElementPredicate<'query>) -> Self {
        Self { guard, predicate }
    }

    pub(super) fn generate_transitions_from_string(query: &'query str) -> Vec<Self> {
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
        current_depth: crate::selection_engine::DepthSize,
        last_depth: crate::selection_engine::DepthSize,
    ) -> bool {
        assert!(
            current_depth >= last_depth,
            "Current depth is smaller than last depth: {current_depth} >= {last_depth}"
        );

        self.guard.evaluate(last_depth, current_depth) && &self.predicate == element
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn back<'html>(
        &self,
        _element: &'html str,
        current_depth: crate::selection_engine::DepthSize,
        last_depth: crate::selection_engine::DepthSize,
    ) -> bool {
        // dbg_print!("'{last_depth}' == '{current_depth}'");
        last_depth == current_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_next_descendant() {
        let state = Transition::new(
            Combinator::Descendant,
            ElementPredicate {
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
                attributes: &[]
            },
            4,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_child() {
        let state = Transition::new(
            Combinator::Child,
            ElementPredicate {
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
                attributes: &[]
            },
            2,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_child_failed() {
        let state = Transition::new(
            Combinator::Child,
            ElementPredicate {
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
                attributes: &[]
            },
            4,
            1,
        ));
    }

    #[test]
    fn test_fsm_next_nextsibling() {
        let state = Transition::new(
            Combinator::NextSibling,
            ElementPredicate {
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
                attributes: &[]
            },
            1,
            1,
        ));
    }
    #[test]
    fn test_fsm_next_subsequentsiblings() {
        let state = Transition::new(
            Combinator::SubsequentSibling,
            ElementPredicate {
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
                attributes: &[]
            },
            1,
            1,
        ));
    }
}
