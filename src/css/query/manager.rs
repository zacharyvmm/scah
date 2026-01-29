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
type Runner<'query> = Vec<SelectionRunner<'query, 'query>>;

#[derive(Debug)]
pub struct FsmManager<'query> {
    runners: Runner<'query>,
}

impl<'html, 'query: 'html> FsmManager<'query> {
    pub fn new(queries: &'query [Query<'query>]) -> Self {
        Self {
            runners: queries
                .iter()
                .map(|query| SelectionRunner::new(query))
                .collect::<Runner<'query>>(),
        }
    }

    pub(crate) fn next(
        &mut self,
        xhtml_element: &XHtmlElement<'html>,
        position: &DocumentPosition,
        store: &mut Store<'html, 'query>,
    ) {
        for session in self.runners.iter_mut() {
            let _ = session.next(&xhtml_element, position, store);
        }
    }

    pub(crate) fn back(
        &mut self,
        xhtml_element: &'html str,
        position: &DocumentPosition,
        reader: &crate::utils::Reader<'html>,
        store: &mut Store<'html, 'query>,
    ) -> bool {
        let mut remove_indices = vec![];
        for (index, session) in self.runners.iter_mut().enumerate() {
            let early_exit = session.early_exit();
            let back = session.back(store, xhtml_element, position, reader);

            if early_exit && back {
                remove_indices.push(index);
            }
        }
        for idx in remove_indices {
            self.runners.remove(idx);
        }

        self.runners.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::{FsmManager, Query, Save, Store};

    use super::super::selection::SelectionRunner;
    use smallvec::SmallVec;

    #[test]
    fn runner_size() {
        println!(
            "Vec size: {}",
            std::mem::size_of::<Vec<SelectionRunner<'static, 'static>>>()
        );
        println!(
            "Inline size: {}",
            std::mem::size_of::<SmallVec<[SelectionRunner<'static, 'static>; 1]>>()
        );
    }

    #[test]
    fn test_single_element_query() {
        let query = Query::first("a", Save::all()).build();
        let q = &[query];
        let manager = FsmManager::new(q);
    }
}
