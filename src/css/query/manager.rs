use super::selection::Selection;
use crate::XHtmlElement;

pub struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: usize,
}

struct FsmManager<'query, 'html> {
    list: Vec<XHtmlElement<'html>>,
    sessions: Vec<Selection<'query, 'html>>,
}

impl<'query, 'html> FsmManager<'query, 'html> {
    fn new() -> Self {
        Self {
            list: Vec::new(),
            sessions: Vec::new(),
        }
    }

    fn next(&'html mut self, xhtml_element: XHtmlElement<'html>, position: DocumentPosition) {
        for session in self.sessions.iter_mut() {
            session.next(&xhtml_element, &position);
        }
    }

    fn step_back(depth: usize) {}
}
