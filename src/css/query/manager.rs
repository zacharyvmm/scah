use super::selection::Selection;
use crate::XHtmlElement;
use crate::css::parser::tree::SelectionTree;
use crate::css::query::tree::MatchTree;

pub(crate) struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: usize,
}

#[derive(Debug)]
pub struct FsmManager<'html, 'query:'html> {
    sessions: Vec<Selection<'query, 'html>>,
}

impl<'html, 'query:'html> FsmManager<'html, 'query> {
    pub fn new(queries: &'query Vec<SelectionTree<'query>>) -> Self {
        Self {
            sessions: queries.iter().map(|query| Selection::new(query)).collect(),
        }
    }

    pub fn next(&mut self, xhtml_element: XHtmlElement<'html>, position: &DocumentPosition) {
        for session in self.sessions.iter_mut() {
            session.next(&xhtml_element, position);
        }
    }

    pub fn back(&mut self, xhtml_element: &'html str, position: &DocumentPosition) {
        for session in self.sessions.iter_mut() {
            session.back(&xhtml_element, position);
        }
    }

    pub fn matches(self) -> Vec<MatchTree<'html>> {
        self.sessions.into_iter().map(|selection| selection.matches()).collect()
    }
}
