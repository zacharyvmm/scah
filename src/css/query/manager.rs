use super::selection::Selection;
use crate::XHtmlElement;

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

    fn next(&'html mut self, xhtml_element: XHtmlElement<'html>, depth: usize) {
        for session in self.sessions.iter_mut() {
            session.next(&xhtml_element, depth);
        }
    }

    fn step_back(depth: usize) {}
}
