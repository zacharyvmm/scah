use std::fmt::Debug;

use super::selection::SelectionRunner;
use crate::XHtmlElement;
use crate::css::parser::tree::Query;
use crate::store::Store;

pub(crate) struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: crate::css::query::DepthSize,
}

//type Runner<'query, E> = SmallVec<[SelectionRunner<'query, 'query, E>; 1]>;
type Runner<'query, E> = Vec<SelectionRunner<'query, 'query, E>>;

#[derive(Debug)]
pub struct FsmManager<'html, 'query: 'html, S>
where
    S: Store<'html, 'query>,
    S::E: Debug + Copy + Default,
{
    store: S,
    runners: Runner<'query, S::E>,
}

impl<'html, 'query: 'html, S, E> FsmManager<'html, 'query, S>
where
    S: Store<'html, 'query, E = E>,
    E: Debug + Copy + Default + Eq,
{
    pub fn new(s: S, queries: &'query [Query<'query>]) -> Self {
        // BUG: the memory moves afterwards
        Self {
            runners: queries
                .iter()
                .map(|query| SelectionRunner::new(query))
                .collect::<Runner<'query, S::E>>(),
            store: s,
        }
    }

    pub(crate) fn next(
        &mut self,
        xhtml_element: &XHtmlElement<'html>,
        position: &DocumentPosition,
    ) {
        for session in self.runners.iter_mut() {
            let _ = session.next(&xhtml_element, position, &mut self.store);
        }
    }

    pub(crate) fn back(
        &mut self,
        xhtml_element: &'html str,
        position: &DocumentPosition,
        reader: &crate::utils::Reader<'html>,
        content: &crate::xhtml::text_content::TextContent,
    ) -> bool {
        let mut remove_indices = vec![];
        for (index, session) in self.runners.iter_mut().enumerate() {
            let early_exit = session.early_exit();
            let back = session.back(&mut self.store, xhtml_element, position, reader, content);

            if early_exit && back {
                remove_indices.push(index);
            }
        }
        for idx in remove_indices {
            self.runners.remove(idx);
        }

        self.runners.is_empty()
    }

    pub fn matches(self) -> S {
        self.store
    }
}

#[cfg(test)]
mod tests {
    use crate::{FsmManager, Query, RustStore, Save, Store};

    use super::super::selection::SelectionRunner;
    use smallvec::SmallVec;

    #[test]
    fn runner_size() {
        println!(
            "Vec size: {}",
            std::mem::size_of::<Vec<SelectionRunner<'static, 'static, usize>>>()
        );
        println!(
            "Inline size: {}",
            std::mem::size_of::<SmallVec<[SelectionRunner<'static, 'static, usize>; 1]>>()
        );
    }

    #[test]
    fn test_single_element_query() {
        let query = Query::first("a", Save::all()).build();
        let q = &[query];
        let manager = FsmManager::new(RustStore::new(()), q);
    }
}
