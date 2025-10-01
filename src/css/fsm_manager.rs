use super::fsm_session::{Element, FsmSession};
use crate::XHtmlElement;

struct FsmManager<'query, 'html> {
    list: Vec<XHtmlElement<'html>>,
    sessions: Vec<FsmSession<'query, 'html>>,
}

impl<'query, 'html> FsmManager<'query, 'html> {
    fn new() -> Self {
        Self {
            list: Vec::new(),
            sessions: Vec::new(),
        }
    }

    fn step_foward(&'html mut self, depth: usize, xhtml_element: XHtmlElement<'html>) {
        let element = Element::Element(xhtml_element);
        for session in self.sessions.iter_mut() {
            session.step_foward(depth, &mut element);
        }
    }

    fn step_back(depth: usize) {}
}
