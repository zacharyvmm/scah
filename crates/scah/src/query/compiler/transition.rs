use crate::XHtmlElement;
use crate::query::compiler::SelectorParseError;
use crate::query::selector::{Combinator, ElementPredicate, Lexer};
use crate::support::Reader;

#[derive(PartialEq, Debug, Clone)]
pub struct Transition<'query> {
    pub guard: Combinator, // from transition
    pub predicate: ElementPredicate<'query>,
}

impl<'query> Transition<'query> {
    pub fn new(guard: Combinator, predicate: ElementPredicate<'query>) -> Self {
        Self { guard, predicate }
    }

    pub(super) fn generate_transitions_from_string(
        query: &'query str,
    ) -> Result<Vec<Self>, SelectorParseError> {
        let reader = &mut Reader::new(query);
        let mut states = Vec::new();
        let mut seen_selector = false;
        while let Some((combinator, element)) = Lexer::try_next(reader, seen_selector)? {
            seen_selector = true;
            states.push(Self::new(combinator, element));
        }

        if states.is_empty() {
            return Err(SelectorParseError::new("empty selector", 0));
        }

        Ok(states)
    }

    pub fn next(
        &self,
        element: &XHtmlElement,
        current_depth: crate::engine::DepthSize,
        last_depth: crate::engine::DepthSize,
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
        current_depth: crate::engine::DepthSize,
        last_depth: crate::engine::DepthSize,
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
