//! Struct-of-arrays index of every resident cursor's tag requirement.

use super::DepthSize;
use super::multiplexer::RunnerId;
use crate::__private::PredicateMetadata;
use scah_query_ir::TagId;

/// Hot-column key for entries every opening tag must visit: transitions
/// without a tag name, and adjacent-sibling watchers that expire on any tag.
const ANY_TAG: u8 = u8::MAX;
/// Hot-column key for blocked or complete cursors; matches no tag.
const INACTIVE: u8 = u8::MAX - 1;
const _: () = assert!(TagId::COUNT <= INACTIVE as usize);

/// Scan width: each chunk of a column compares into one bitmask, which the
/// compiler can vectorize, and only set bits are visited.
const CHUNK: usize = 16;

/// Bit `i` is set when `keep(chunk[i])`.
#[inline(always)]
fn chunk_mask<T: Copy>(chunk: &[T; CHUNK], keep: impl Fn(T) -> bool) -> u32 {
    let mut mask = 0;
    for (index, &value) in chunk.iter().enumerate() {
        mask |= u32::from(keep(value)) << index;
    }
    mask
}

/// Calls `visit` with the index of every element of `values[..len]` that
/// satisfies `keep`, in order. Full chunks go through [`chunk_mask`]; the
/// tail, which is the whole column for small query sets, is compared directly
/// because building a mask costs more than a few scalar compares. `visit`
/// gets `state` back mutably, so `values` is re-read from it per chunk.
#[inline(always)]
fn for_each_match<S: ?Sized, T: Copy>(
    state: &mut S,
    values: impl Fn(&S) -> &[T],
    keep: impl Fn(T) -> bool,
    mut visit: impl FnMut(&mut S, usize),
) {
    let len = values(state).len();
    let full = len - len % CHUNK;
    let mut start = 0;
    while start < full {
        let chunk = <&[T; CHUNK]>::try_from(&values(state)[start..start + CHUNK])
            .expect("chunk has CHUNK elements");
        let mut mask = chunk_mask(chunk, &keep);
        while mask != 0 {
            visit(state, start + mask.trailing_zeros() as usize);
            mask &= mask - 1;
        }
        start += CHUNK;
    }
    for index in full..len {
        if keep(values(state)[index]) {
            visit(state, index);
        }
    }
}

/// Preflight data for one cursor, fixed when the cursor is pushed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CursorTarget<'query> {
    pub metadata: &'query PredicateMetadata<'query>,
    /// Viable save point that stores every attribute.
    pub requires_all: bool,
    pub adjacent: bool,
}

impl CursorTarget<'_> {
    #[inline]
    fn key(&self) -> u8 {
        match self.metadata.required_tag() {
            Some(tag) if !self.adjacent => tag.index() as u8,
            _ => ANY_TAG,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FrontierEntry<'query> {
    pub runner: u32,
    cursor: u32,
    /// Hot-column key while the cursor is active.
    key: u8,
    pub target: CursorTarget<'query>,
}

/// Receives every cursor lifecycle change made by a query executor.
///
/// Cursor indices follow the executor's `Vec` semantics: `push` appends and
/// `swap_remove` moves the last cursor into the removed index.
pub(crate) trait CursorObserver<'query> {
    fn push(&mut self, runner: RunnerId, target: CursorTarget<'query>, active: bool);
    fn activate(&mut self, runner: RunnerId, cursor: usize);
    fn deactivate(&mut self, runner: RunnerId, cursor: usize);
    fn swap_remove(&mut self, runner: RunnerId, cursor: usize);
    fn clear(&mut self, runner: RunnerId);
    /// Raises the runner's close bound to at least `bound`; see
    /// [`Frontier::needs_close`].
    fn raise_close_bound(&mut self, runner: RunnerId, bound: DepthSize);
    /// Replaces the runner's close bound with an exact recomputation.
    fn set_close_bound(&mut self, runner: RunnerId, bound: DepthSize);
}

/// Standalone executors (unit tests) have no frontier to maintain.
impl<'query> CursorObserver<'query> for () {
    #[inline(always)]
    fn push(&mut self, _: RunnerId, _: CursorTarget<'query>, _: bool) {}
    #[inline(always)]
    fn activate(&mut self, _: RunnerId, _: usize) {}
    #[inline(always)]
    fn deactivate(&mut self, _: RunnerId, _: usize) {}
    #[inline(always)]
    fn swap_remove(&mut self, _: RunnerId, _: usize) {}
    #[inline(always)]
    fn clear(&mut self, _: RunnerId) {}
    #[inline(always)]
    fn raise_close_bound(&mut self, _: RunnerId, _: DepthSize) {}
    #[inline(always)]
    fn set_close_bound(&mut self, _: RunnerId, _: DepthSize) {}
}

/// Every resident cursor across all runners, updated in place as executors
/// push, block, reactivate, complete and remove cursors.
///
/// `tags` is the hot column scanned once per opening tag; `entries` holds
/// the matching cold data at the same index. Blocking and reactivation only
/// rewrite a tag byte. Slots are unordered: removal swaps the last slot into
/// the hole and repoints its owner.
#[derive(Debug, Default)]
pub(crate) struct Frontier<'query> {
    tags: Vec<u8>,
    entries: Vec<FrontierEntry<'query>>,
    /// Per runner, cursor index -> slot.
    slots: Vec<Vec<u32>>,
    /// Per runner, an upper bound on one past the deepest close that can
    /// change any of its cursors. Raised as cursors gain scopes or pending
    /// closes, and made exact whenever the runner handles a close.
    close_bounds: Vec<DepthSize>,
}

impl<'query> Frontier<'query> {
    pub(crate) fn with_runners(runner_count: usize) -> Self {
        Self {
            tags: Vec::new(),
            entries: Vec::new(),
            slots: vec![Vec::new(); runner_count],
            close_bounds: vec![0; runner_count],
        }
    }

    /// Whether closing an element at `depth` can change `runner`'s cursors.
    #[inline(always)]
    pub(crate) fn needs_close(&self, runner: RunnerId, depth: DepthSize) -> bool {
        self.close_bounds[runner.index()] > depth
    }

    /// Calls `visit` with every runner whose cursors a close at `depth` can
    /// change, in runner order. `visit` receives the frontier back so the
    /// runner's close step can update it.
    #[inline(always)]
    pub(crate) fn for_each_needing_close(
        &mut self,
        depth: DepthSize,
        mut visit: impl FnMut(&mut Self, RunnerId),
    ) {
        let len = self.close_bounds.len();
        if len >= CHUNK {
            self.for_each_needing_close_chunked(depth, visit);
            return;
        }
        for runner in 0..len {
            if self.close_bounds[runner] > depth {
                visit(self, RunnerId(runner));
            }
        }
    }

    /// Kept out of line so the small-set loop above stays compact; each
    /// visitor inlines a whole close step.
    #[inline(never)]
    fn for_each_needing_close_chunked(
        &mut self,
        depth: DepthSize,
        mut visit: impl FnMut(&mut Self, RunnerId),
    ) {
        let len = self.close_bounds.len();
        let mut start = 0;
        while start < len {
            let end = len.min(start + CHUNK);
            let bounds = &self.close_bounds[start..end];
            let mut mask = match <&[DepthSize; CHUNK]>::try_from(bounds) {
                Ok(chunk) => chunk_mask(chunk, |bound| bound > depth),
                Err(_) => bounds.iter().enumerate().fold(0, |mask, (index, &bound)| {
                    mask | u32::from(bound > depth) << index
                }),
            };
            while mask != 0 {
                visit(self, RunnerId(start + mask.trailing_zeros() as usize));
                mask &= mask - 1;
            }
            start = end;
        }
    }

    /// Calls `visit` with every active cursor that may match an element
    /// resolved to `tag`: entries requiring `tag` and every [`ANY_TAG`] entry.
    /// Callers still confirm with [`PredicateMetadata::matches_tag`] for
    /// nameless and unknown tags.
    #[inline(always)]
    pub(crate) fn for_each_candidate(
        &self,
        tag: TagId,
        mut visit: impl FnMut(&FrontierEntry<'query>),
    ) {
        let key = tag.index() as u8;
        for_each_match(
            &mut &*self,
            |frontier| &frontier.tags,
            |candidate| candidate == key || candidate == ANY_TAG,
            |frontier, slot| visit(&frontier.entries[slot]),
        );
    }

    #[inline]
    fn slot(&self, runner: RunnerId, cursor: usize) -> usize {
        self.slots[runner.index()][cursor] as usize
    }

    #[inline]
    fn remove_slot(&mut self, slot: usize) {
        self.tags.swap_remove(slot);
        self.entries.swap_remove(slot);
        if let Some(moved) = self.entries.get(slot) {
            self.slots[moved.runner as usize][moved.cursor as usize] = slot as u32;
        }
    }

    /// Number of active cursors.
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn active_len(&self) -> usize {
        self.tags.iter().filter(|&&tag| tag != INACTIVE).count()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn is_active(&self, runner: RunnerId, cursor: usize) -> bool {
        self.tags[self.slot(runner, cursor)] != INACTIVE
    }

    #[cfg(debug_assertions)]
    pub(crate) fn resident_len(&self, runner: RunnerId) -> usize {
        self.slots[runner.index()].len()
    }
}

impl<'query> CursorObserver<'query> for Frontier<'query> {
    #[inline]
    fn push(&mut self, runner: RunnerId, target: CursorTarget<'query>, active: bool) {
        let slots = &mut self.slots[runner.index()];
        let key = target.key();
        self.tags.push(if active { key } else { INACTIVE });
        self.entries.push(FrontierEntry {
            runner: runner.index() as u32,
            cursor: slots.len() as u32,
            key,
            target,
        });
        slots.push(self.tags.len() as u32 - 1);
    }

    #[inline]
    fn activate(&mut self, runner: RunnerId, cursor: usize) {
        let slot = self.slot(runner, cursor);
        self.tags[slot] = self.entries[slot].key;
    }

    #[inline]
    fn deactivate(&mut self, runner: RunnerId, cursor: usize) {
        let slot = self.slot(runner, cursor);
        self.tags[slot] = INACTIVE;
    }

    #[inline]
    fn swap_remove(&mut self, runner: RunnerId, cursor: usize) {
        let slot = self.slot(runner, cursor);
        self.remove_slot(slot);
        let slots = &mut self.slots[runner.index()];
        slots.swap_remove(cursor);
        if let Some(&moved) = slots.get(cursor) {
            self.entries[moved as usize].cursor = cursor as u32;
        }
    }

    fn clear(&mut self, runner: RunnerId) {
        // Pop from the end so a slot moved by `remove_slot` always belongs to
        // a cursor index that is still resident.
        while let Some(slot) = self.slots[runner.index()].pop() {
            self.remove_slot(slot as usize);
        }
        self.close_bounds[runner.index()] = 0;
    }

    #[inline]
    fn raise_close_bound(&mut self, runner: RunnerId, bound: DepthSize) {
        let current = &mut self.close_bounds[runner.index()];
        *current = (*current).max(bound);
    }

    #[inline]
    fn set_close_bound(&mut self, runner: RunnerId, bound: DepthSize) {
        self.close_bounds[runner.index()] = bound;
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorObserver, CursorTarget, Frontier};
    use crate::engine::multiplexer::RunnerId;
    use crate::{Query, QuerySpec, Save};
    use scah_query_ir::TagId;

    fn runners_at<'query>(frontier: &Frontier<'query>, tag: TagId) -> Vec<u32> {
        let mut runners = Vec::new();
        frontier.for_each_candidate(tag, |entry| runners.push(entry.runner));
        runners.sort_unstable();
        runners
    }

    #[test]
    fn removal_repoints_moved_slots_across_runners() {
        let query = Query::all("div", Save::none()).unwrap().build();
        let target = CursorTarget {
            metadata: query.states()[0].metadata(),
            requires_all: false,
            adjacent: false,
        };
        let mut frontier = Frontier::with_runners(2);
        for runner in [RunnerId(0), RunnerId(1), RunnerId(0)] {
            frontier.push(runner, target, true);
        }
        assert_eq!(runners_at(&frontier, TagId::DIV), [0, 0, 1]);
        assert!(runners_at(&frontier, TagId::SPAN).is_empty());

        // Removing runner 0's first cursor moves both a slot and a cursor.
        frontier.swap_remove(RunnerId(0), 0);
        assert_eq!(runners_at(&frontier, TagId::DIV), [0, 1]);
        frontier.deactivate(RunnerId(0), 0);
        assert_eq!(runners_at(&frontier, TagId::DIV), [1]);
        frontier.activate(RunnerId(0), 0);
        frontier.clear(RunnerId(1));
        assert_eq!(runners_at(&frontier, TagId::DIV), [0]);
        frontier.clear(RunnerId(0));
        assert_eq!(frontier.active_len(), 0);
        assert!(frontier.tags.is_empty());
    }
}
