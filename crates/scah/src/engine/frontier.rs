//! Struct-of-arrays index of every resident cursor's tag requirement.

use super::multiplexer::RunnerId;
use crate::__private::PredicateMetadata;
use scah_query_ir::TagId;

/// Hot-column key for entries every opening tag must visit: transitions
/// without a tag name, and adjacent-sibling watchers that expire on any tag.
const ANY_TAG: u8 = u8::MAX;
/// Hot-column key for blocked or complete cursors; matches no tag.
const INACTIVE: u8 = u8::MAX - 1;
const _: () = assert!(TagId::COUNT <= INACTIVE as usize);

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
}

impl<'query> Frontier<'query> {
    pub(crate) fn with_runners(runner_count: usize) -> Self {
        Self {
            tags: Vec::new(),
            entries: Vec::new(),
            slots: vec![Vec::new(); runner_count],
        }
    }

    /// Active cursors that may match an element resolved to `tag`.
    ///
    /// Yields every entry whose required tag equals `tag` plus every
    /// [`ANY_TAG`] entry; callers still confirm with
    /// [`PredicateMetadata::matches_tag`] for nameless and unknown tags.
    #[inline(always)]
    pub(crate) fn candidates(&self, tag: TagId) -> impl Iterator<Item = &FrontierEntry<'query>> {
        let key = tag.index() as u8;
        self.tags
            .iter()
            .zip(&self.entries)
            .filter(move |(candidate, _)| **candidate == key || **candidate == ANY_TAG)
            .map(|(_, entry)| entry)
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
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorObserver, CursorTarget, Frontier};
    use crate::engine::multiplexer::RunnerId;
    use crate::{Query, QuerySpec, Save};
    use scah_query_ir::TagId;

    fn runners_at<'query>(frontier: &Frontier<'query>, tag: TagId) -> Vec<u32> {
        let mut runners: Vec<_> = frontier.candidates(tag).map(|entry| entry.runner).collect();
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
