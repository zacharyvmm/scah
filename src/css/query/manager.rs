use std::fmt::Debug;
use std::marker::PhantomData;

use super::selection::SelectionRunner;
use crate::XHtmlElement;
use crate::css::parser::tree::Query;
use crate::store::Store;
use crate::store::reserve::{Fsm, Reserve};

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
    S::E: Debug + Copy + Default
{
    store: S,
    runners: Runner<'query, S::E>,
    reserve: Reserve<S::E>,
    index: &'html u8,
}

impl<'html, 'query: 'html, S, E> FsmManager<'html, 'query, S>
where
    S: Store<'html, 'query, E = E>,
    E: Debug + Copy + Default + Eq
{
    pub fn new(s: S, queries: &'query [Query<'query>]) -> Self {
        // BUG: the memory moves afterwards
        Self {
            runners: queries
                .iter()
                .map(|query| SelectionRunner::new(query))
                .collect::<Runner<'query, S::E>>(),
            store: s,
            reserve: Reserve::new(),
            index: &0,
        }
    }

    fn save_element_from_reservation(
        &mut self,
        reservation: Fsm<E>,
        xhtml_element: XHtmlElement<'html>,
        position: &DocumentPosition,
    ) {
        let runner = &mut self.runners[reservation.section];

        let _ = match reservation.index {
            None => {
                println!("Saving too FSM");
                SelectionRunner::save_element(
                    &mut runner.on_close_tag_events,
                    runner.selection_tree,
                    &mut self.store,
                    xhtml_element,
                    position,
                    &mut runner.fsm,
                )
            }
            Some(i) => {
                println!("Saving too ScopedFSM {i}");
                SelectionRunner::save_element(
                    &mut runner.on_close_tag_events,
                    runner.selection_tree,
                    &mut self.store,
                    xhtml_element,
                    position,
                    &mut runner.scoped_fsms[i],
                )
            }
        };
    }

    pub fn next(&mut self, xhtml_element: XHtmlElement<'html>, position: &DocumentPosition) {
        for (index, session) in self.runners.iter_mut().enumerate() {
            self.reserve.set_section(index);
            let _ = session.next(&xhtml_element, position, &mut self.reserve, &mut self.store);
        }

        match self.reserve.list.len() {
            0 => {}
            1 => {
                let reservation = self.reserve.list.pop().unwrap();
                self.save_element_from_reservation(reservation, xhtml_element, position);
            }
            len => {
                for _ in 0..len - 2 {
                    let reservation = self.reserve.list.pop().unwrap();
                    self.save_element_from_reservation(
                        reservation,
                        xhtml_element.clone(),
                        position,
                    );
                }

                let reservation = self.reserve.list.pop().unwrap();
                self.save_element_from_reservation(reservation, xhtml_element, position);
            }
        };
    }

    pub fn back(
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

mod tests {
    use super::super::selection::SelectionRunner;
    use crate::Element;
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
}
