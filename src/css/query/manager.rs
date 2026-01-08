use smallvec::SmallVec;

use super::selection::SelectionRunner;
use crate::XHtmlElement;
use crate::css::parser::tree::Query;
//use crate::css::query::tree::MatchTree;
use crate::store::Store;

pub(crate) struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: crate::css::query::DepthSize,
}

//type Runner<'query, E> = SmallVec<[SelectionRunner<'query, E>; 1]>;
type Runner<'query, E> = Vec<SelectionRunner<'query, 'query, E>>;

#[derive(Debug)]
pub struct FsmManager<'html, 'query: 'html, S>
where
    S: Store<'html, 'query>,
{
    store: S,
    sessions: Runner<'query, S::E>,
}

impl<'html, 'query: 'html, S, E> FsmManager<'html, 'query, S>
where
    S: Store<'html, 'query, E = E>,
{
    pub fn new(mut s: S, queries: &'query [Query<'query>]) -> Self {
        // BUG: the memory moves afterwards
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
            let _ = session.next(&mut self.store, &xhtml_element, position);
        }
    }

    pub fn back(
        &mut self,
        xhtml_element: &'html str,
        position: &DocumentPosition,
        reader: &crate::utils::Reader<'html>,
        content: &crate::xhtml::text_content::TextContent,
    ) -> bool {
        let mut remove_indices = vec![];
        for (index, session) in self.sessions.iter_mut().enumerate() {
            let early_exit = session.early_exit();
            let back = session.back(&mut self.store, xhtml_element, position, reader, content);

            if early_exit && back {
                remove_indices.push(index);
            }
        }
        for idx in remove_indices {
            self.sessions.remove(idx);
        }

        self.sessions.is_empty()
    }

    pub fn matches(self) -> S {
        self.store
    }
}
