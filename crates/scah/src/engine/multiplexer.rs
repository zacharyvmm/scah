use super::executor::QueryExecutor;
use crate::XHtmlElement;
use crate::store::ElementId;
use crate::store::Store;
use crate::{QuerySpec, Reader};

pub(crate) struct DocumentPosition {
    pub reader_position: usize,
    pub text_content_position: usize,
    pub element_depth: crate::engine::DepthSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SaveHit {
    pub element_id: ElementId,
    pub save_inner_html: bool,
    pub save_text_content: bool,
}

//type Runner<'query, Q> = SmallVec<[QueryExecutor<'query, Q>; 1]>;
type Runner<'query, Q> = Vec<QueryExecutor<'query, Q>>;

pub struct QueryMultiplexer<'query, Q> {
    runners: Runner<'query, Q>,
}

impl<'html, 'query: 'html, Q> QueryMultiplexer<'query, Q>
where
    Q: QuerySpec<'query>,
{
    pub fn new(queries: &'query [Q]) -> Self {
        Self {
            #[allow(clippy::redundant_closure)]
            runners: queries
                .iter()
                .map(|query| QueryExecutor::new(query))
                .collect::<Runner<'query, Q>>(),
        }
    }

    pub(crate) fn next(
        &mut self,
        xhtml_element: &XHtmlElement<'html>,
        position: &DocumentPosition,
        store: &mut Store<'html, 'query>,
    ) -> Vec<SaveHit> {
        let len = store.elements.len();
        let mut save_hits = Vec::new();
        for session in self.runners.iter_mut() {
            session.next(xhtml_element, position, store, &mut save_hits);
        }
        if len == store.elements.len() {
            // Element was not saved
            // Thus delete from the tape
            xhtml_element.remove_attributes(&mut store.attributes);
        }
        save_hits
    }

    pub(crate) fn back(
        &mut self,
        xhtml_element: &'html str,
        position: &DocumentPosition,
        _reader: &Reader<'html>,
    ) -> bool {
        let mut remove_indices = vec![];
        for (index, session) in self.runners.iter_mut().enumerate() {
            let early_exit = session.early_exit();
            let back = session.back(xhtml_element, position);

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
