//! Allocations of dropped stores, kept for the next parse on the same thread.
//!
//! A parse fills a handful of vectors whose size follows the document.
//! Allocating them afresh for every document costs more than the copies
//! suggest: the allocator hands freed memory back to the OS and the next
//! parse page-faults it in again. Keeping the vectors of the last dropped
//! store per thread makes repeated parses allocate only when a document
//! needs more room than any before it.

use std::cell::Cell;

use super::arena::id::ElementId;
use super::columns::Columns;
use super::text::TextStore;

/// Largest set of allocations kept for reuse. A store whose buffers are
/// bigger is freed normally, so one huge document does not pin its memory
/// for the life of the thread.
pub(crate) const MAX_RETAINED_BYTES: usize = 32 << 20;

/// The vectors of a store, emptied. Strings are spans or are cleared, so
/// nothing here borrows the document it came from.
#[derive(Debug, Default)]
pub(crate) struct StoreBuffers {
    pub(crate) columns: Columns,
    pub(crate) text: TextStore,
    pub(crate) edges: Vec<(u32, u32)>,
    pub(crate) slot_starts: Vec<u32>,
    pub(crate) slot_offsets: Vec<u32>,
    pub(crate) results: Vec<ElementId>,
}

impl StoreBuffers {
    /// Heap bytes these buffers hold.
    pub(crate) fn bytes(&self) -> usize {
        fn bytes<T>(vec: &Vec<T>) -> usize {
            vec.capacity() * std::mem::size_of::<T>()
        }
        let columns = &self.columns;
        bytes(&columns.rows)
            + bytes(&columns.attribute_keys)
            + bytes(&columns.attribute_values)
            + bytes(&columns.inner_html)
            + bytes(&columns.raw_text)
            + bytes(&columns.text)
            + self.text.raw_text.capacity()
            + self.text.text.capacity()
            + bytes(&self.edges)
            + bytes(&self.slot_starts)
            + bytes(&self.slot_offsets)
            + bytes(&self.results)
    }
}

thread_local! {
    static SPARE: Cell<Option<StoreBuffers>> = const { Cell::new(None) };
}

/// The buffers kept on this thread, if any.
pub(crate) fn take() -> Option<StoreBuffers> {
    SPARE.try_with(Cell::take).ok().flatten()
}

/// Keep `buffers` for the next parse on this thread, unless they are too
/// large or the buffers already kept are larger.
pub(crate) fn keep(buffers: StoreBuffers) {
    let size = buffers.bytes();
    if size == 0 || size > MAX_RETAINED_BYTES {
        return;
    }
    // The thread may be exiting, in which case the buffers are just freed.
    let _ = SPARE.try_with(|spare| {
        let kept = spare.take();
        let keep = match kept {
            Some(kept) if kept.bytes() >= size => kept,
            _ => buffers,
        };
        spare.set(Some(keep));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffers(rows: usize) -> StoreBuffers {
        let mut buffers = StoreBuffers::default();
        buffers.columns.rows.reserve_exact(rows);
        buffers
    }

    #[test]
    fn keeps_the_larger_buffers_up_to_the_cap() {
        assert!(take().is_none());
        keep(buffers(10));
        keep(buffers(100));
        keep(buffers(50));
        let kept = take().unwrap();
        assert!(kept.columns.rows.capacity() >= 100);
        assert!(take().is_none());

        let row = std::mem::size_of::<super::super::columns::RowHead>();
        keep(buffers(MAX_RETAINED_BYTES / row + 1));
        assert!(take().is_none(), "buffers past the cap are freed");
        keep(StoreBuffers::default());
        assert!(take().is_none(), "nothing to keep");
    }
}
