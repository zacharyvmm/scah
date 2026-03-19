use super::{ElementId, QueryId};
use std::ops::{Bound, RangeBounds};

#[derive(PartialEq, Debug, Default)]
pub struct Span<Idx> {
    start: Idx,
    end: Idx,
}

impl<T: Copy + PartialOrd> Span<T> {
    pub fn new(start: T) -> Self {
        Self { start, end: start }
    }

    pub(super) fn from(start: T, end: T) -> Self {
        Self { start, end }
    }

    pub fn start(&self) -> T {
        self.start
    }

    pub fn end(&self) -> T {
        self.end
    }

    pub fn set_end(&mut self, value: T) {
        assert!(self.start <= value);
        self.end = value
    }
}

impl<T> RangeBounds<T> for Span<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }

    fn end_bound(&self) -> Bound<&T> {
        Bound::Excluded(&self.end)
    }
}

pub type ElementSpan = Span<ElementId>;
type QuerySpan = Span<QueryId>;
type TextContentSpan = Span<usize>;
