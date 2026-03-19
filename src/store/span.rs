use super::{ElementId, QueryId};
use std::ops::{Bound, RangeBounds};

pub struct Span<Idx> {
    start: Idx,
    end: Idx,
}

impl<T> Span<T> {
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
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

type ElementSpan = Span<ElementId>;
type QuerySpan = Span<QueryId>;
type TextContentSpan = Span<usize>;
