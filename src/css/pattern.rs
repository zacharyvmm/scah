use super::parser::query_tokenizer::Combinator;
use super::state::{Fsm, SelectionKind};
use crate::XHtmlElement;

// This is the statefull part of the Fsms, thus this is a light wrapper
#[derive(PartialEq, Debug, Clone)]
pub struct Pattern {
    pub(crate) parent_save_position: usize, // in the tree
    pub(crate) position: usize,
    pub(super) depths: Vec<usize>, // Depths since the last save position
}

impl Pattern {
    pub fn new() -> Self {
        Self {
            parent_save_position: 0,
            position: 0,
            depths: Vec::new(),
        }
    }

    pub fn next<'html>(
        &self,
        fsms: &Vec<Fsm>,
        depth: usize,
        element: &XHtmlElement<'html>,
    ) -> bool {
        let last_depth = {
            let last = self.depths.last();
            if last.is_some() { *(last.unwrap()) } else { 0 }
        };

        let next = fsms[self.position].next(depth, last_depth, element);

        if !next {
            if (fsms[self.position].transition == Combinator::NextSibling
                || fsms[self.position].transition == Combinator::SubsequentSibling)
                && matches!(fsms[self.position - 1].state_kind, SelectionKind::First(..))
            {
                // Pattern Failed to select, thus this needs to be killed
            }
        }

        return next;
    }

    pub fn back<'html>(&self, fsms: &Vec<Fsm>, depth: usize, element: &'html str) -> bool {
        let last_depth = {
            let last = self.depths.last();
            if last.is_some() { *(last.unwrap()) } else { 0 }
        };

        let back = fsms[self.position].back(depth, last_depth, element);

        return back;
    }

    #[inline]
    pub fn retry(&self, fsms: &Vec<Fsm>) -> bool {
        fsms[self.position].retry()
    }

    #[inline]
    pub fn is_reset(&self) -> bool {
        return self.position == 0 && self.depths.len() == 0;
    }

    pub fn move_foward(&mut self, depth: usize) {
        self.depths.push(depth);
        self.position += 1;
    }

    pub fn move_backward(&mut self) {
        self.depths.pop();
        self.position -= 1;
    }
}
