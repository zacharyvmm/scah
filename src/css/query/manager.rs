use super::selection::SelectionRunner;
use crate::XHtmlElement;
use crate::css::parser::tree::Selection;
//use crate::css::query::tree::MatchTree;
use crate::store::Store;

pub(crate) struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: usize,
}

#[derive(Debug)]
pub struct FsmManager<'html, 'query: 'html, S>
where
    S: Store<'html, 'query>,
{
    store: S,
    sessions: Vec<SelectionRunner<'query, S::E>>,
}

impl<'html, 'query: 'html, S, E> FsmManager<'html, 'query, S>
where
    S: Store<'html, 'query, E = E>,
{
    pub fn new(queries: &'query Vec<Selection<'query>>) -> Self {
        // BUG: the memory moves afterwards
        let mut s = S::new();
        Self {
            sessions: queries
                .iter()
                .map(|query| SelectionRunner::new(s.root(), query))
                .collect(),
            store: s,
        }
    }

    pub fn next(&mut self, xhtml_element: XHtmlElement<'html>, position: &DocumentPosition) {
        for session in self.sessions.iter_mut() {
            session.next(&mut self.store, &xhtml_element, position);
        }
    }

    pub fn back(
        &mut self,
        xhtml_element: &'html str,
        position: &DocumentPosition,
        reader: &crate::utils::Reader<'html>,
        content: &crate::xhtml::text_content::TextContent,
    ) {
        for session in self.sessions.iter_mut() {
            session.back(&mut self.store, &xhtml_element, position, reader, content);
        }
    }

    pub fn matches(self) -> S {
        self.store
    }
}
