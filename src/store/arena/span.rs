use std::ops::Range;

#[derive(PartialEq, Debug, Default)]
pub struct Span<Idx> {
    start: Idx,
    end: Idx,
}

impl<T: Copy + PartialOrd + Into<usize>> Span<T> {
    pub fn new(start: T) -> Self {
        Self { start, end: start }
    }

    pub(crate) fn from(start: T, end: T) -> Self {
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

impl From<Span<u32>> for Range<usize> {
    fn from(value: Span<u32>) -> Self {
        Self {
            start: value.start as usize,
            end: value.end as usize,
        }
    }
}
