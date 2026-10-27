//! Nested sections (`.then(...)`): which parent match a match belongs to.
//!
//! A nested section matches relative to each *scope*: an open element saved
//! by its parent section. The matcher's step bits say that some element
//! matched a step, not under which scope. This module adds that:
//!
//! - **Scope stack.** Saving an element of a section with nested sections
//!   pushes a [`Scope`]; closing the element pops it. While a section has an
//!   open scope, the first steps of its nested sections are candidates
//!   ([`Scopes::entries`]). Each `.then()` level only links to the level above
//!   it, so deeper nesting is just more entries on the same stack.
//! - **Lanes.** For each step of a nested section, the set of open scopes (by
//!   stack slot) that a partial match is anchored in. A match is saved under
//!   every scope in its save step's lane ([`Scopes::parents`]).
//!
//! A lane depends only on where the element is. An entry step's lane comes
//! from the scope stack, a ` ` step's from the lanes its ancestors inherit,
//! and a `>`, `+` or `~` step copies the previous step's lane from its parent
//! or an earlier sibling, which shares the element's position. So only the
//! inherited lanes are stored: one live set, with a log of the bits each
//! matcher frame added so popping the frame can take them out. Every other
//! lane is recomputed when it is read. Memory stays linear in the open
//! scopes, even when every one of thousands of nested scopes anchors a
//! partial match.

use std::ops::Range;

use scah_query_ir::program::bits;
use scah_query_ir::{
    LaneSource, Program, SectionIndex, SectionMask, SelectionKind, StepMask, StepPlan,
};
use smallvec::{SmallVec, smallvec};

use super::matcher::FrameMeta;
use crate::store::ElementId;

/// An open element saved by a section that has nested sections.
#[derive(Debug, Clone)]
struct Scope {
    depth: u32,
    section: SectionIndex,
    /// The element's row for `section`; nested matches are listed under it.
    row: ElementId,
}

/// Bits a frame added to one word of the live inherited lanes.
#[derive(Debug, Clone, Copy)]
struct LaneLogEntry {
    lane: u32,
    word: u32,
    bits: u64,
}

/// Lanes: per scoped step, a bitset over scope slots.
#[derive(Debug)]
struct Lanes {
    /// Number of scoped steps, i.e. lanes per set.
    steps: usize,
    /// Words per lane; grows when more scopes are open than it can index.
    words: usize,
    /// Inherited lanes of the innermost frame: the union of the lanes of
    /// every element with a frame.
    live: SmallVec<[u64; 8]>,
    /// The bits each frame added to `live`, oldest first.
    log: SmallVec<[LaneLogEntry; 8]>,
    /// Lanes of the element being opened.
    current: SmallVec<[u64; 8]>,
}

impl Lanes {
    #[inline]
    fn range(&self, lane: usize) -> Range<usize> {
        let start = lane * self.words;
        start..start + self.words
    }

    /// Add the current element's lanes to the live set, logging new bits.
    fn inherit_current(&mut self) {
        for lane in 0..self.steps {
            for word in 0..self.words {
                let index = lane * self.words + word;
                let bits = self.current[index] & !self.live[index];
                if bits != 0 {
                    self.live[index] |= bits;
                    self.log.push(LaneLogEntry {
                        lane: lane as u32,
                        word: word as u32,
                        bits,
                    });
                }
            }
        }
    }

    /// Remove the bits logged since `mark` from the live set.
    fn undo(&mut self, mark: u32) {
        let mark = mark as usize;
        for entry in &self.log[mark..] {
            self.live[entry.lane as usize * self.words + entry.word as usize] &= !entry.bits;
        }
        self.log.truncate(mark);
    }

    /// Copy `lane` of the live set as it was when the log was `mark` long
    /// into `target` of the current lanes.
    fn inherited_at(&mut self, lane: usize, mark: usize, target: usize) {
        let words = self.words;
        let (source, target) = (lane * words, target * words);
        for word in 0..words {
            self.current[target + word] = self.live[source + word];
        }
        for entry in &self.log[mark..] {
            if entry.lane as usize == lane {
                self.current[target + entry.word as usize] &= !entry.bits;
            }
        }
    }

    /// Make room for `slots` open scopes, widening the lane sets if needed.
    fn ensure_slots(&mut self, slots: usize) {
        if slots <= self.words * 64 {
            return;
        }
        let words = bits::words_for(slots).max(self.words * 2);
        let widen = |old: &SmallVec<[u64; 8]>| -> SmallVec<[u64; 8]> {
            let mut wide = smallvec![0; self.steps * words];
            for lane in 0..self.steps {
                wide[lane * words..lane * words + self.words]
                    .copy_from_slice(&old[lane * self.words..(lane + 1) * self.words]);
            }
            wide
        };
        self.live = widen(&self.live);
        self.current = widen(&self.current);
        self.words = words;
    }
}

/// Open scopes and the lanes that anchor partial matches in them.
#[derive(Debug)]
pub(super) struct Scopes {
    /// Open scopes, outermost first. A scope's index is its slot.
    stack: SmallVec<[Scope; 4]>,
    /// `claim_words` words per scope: which `First` child sections already
    /// matched inside it, by position in the parent's child list.
    claims: SmallVec<[u64; 4]>,
    /// Open scopes of each section.
    open: Box<[u32]>,
    /// [`SectionMask::ChildEntryDescendant`] of every section with an open
    /// scope: the entry steps enabled anywhere below an open scope.
    entry_descendant: Box<[u64]>,
    lanes: Lanes,

    lane_steps: SmallVec<[usize; 8]>,
    slot_scratch: SmallVec<[u64; 2]>,
    parent_scratch: SmallVec<[ElementId; 4]>,
}

impl Scopes {
    pub(super) fn new(program: &Program<'_>) -> Self {
        Self {
            stack: SmallVec::new(),
            claims: SmallVec::new(),
            open: vec![0; program.section_count()].into_boxed_slice(),
            entry_descendant: vec![0; program.words()].into_boxed_slice(),
            lanes: Lanes {
                steps: program.lane_count(),
                words: 1,
                live: smallvec![0; program.lane_count()],
                log: SmallVec::new(),
                current: smallvec![0; program.lane_count()],
            },
            lane_steps: SmallVec::new(),
            slot_scratch: SmallVec::new(),
            parent_scratch: SmallVec::new(),
        }
    }

    #[inline(always)]
    pub(super) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Whether the program has nested sections, so candidates need lanes.
    #[inline(always)]
    pub(super) fn has_lanes(&self) -> bool {
        self.lanes.steps > 0
    }

    /// Word `word` of the nested-section entry steps open scopes enable for
    /// a child of the element at depth `parent`: descendant entries of every
    /// open scope, and child entries of the scopes at `parent` itself.
    #[inline(always)]
    pub(super) fn entries(
        &self,
        program: &Program<'_>,
        parent: usize,
        word: usize,
        words: usize,
    ) -> u64 {
        let mut entries = self.entry_descendant[word];
        for scope in self.stack.iter().rev() {
            if scope.depth as usize != parent {
                break;
            }
            entries |=
                program.section_mask_word(scope.section, SectionMask::ChildEntryChild, word, words);
        }
        entries
    }

    /// Words of lane state held: live lanes plus logged words.
    #[cfg(test)]
    pub(super) fn lane_words(&self) -> usize {
        self.lanes.live.len() + self.lanes.log.len()
    }

    /// Length of the lane log, to record on a new matcher frame.
    #[inline(always)]
    pub(super) fn log_mark(&self) -> u32 {
        self.lanes.log.len() as u32
    }

    /// The element being opened gets a matcher frame: its lanes become part
    /// of what its descendants inherit.
    #[inline(always)]
    pub(super) fn inherit_current(&mut self) {
        self.lanes.inherit_current();
    }

    /// A matcher frame recorded at `mark` is popped.
    #[inline(always)]
    pub(super) fn undo(&mut self, mark: u32) {
        self.lanes.undo(mark);
    }

    /// Fill lanes for the scoped `candidates` of an element opened at
    /// `depth`, and drop candidates no scope can anchor.
    #[inline(never)]
    pub(super) fn anchor(
        &mut self,
        program: &Program<'_>,
        candidates: &mut [u64],
        depth: usize,
        frames: &[FrameMeta],
    ) {
        let scoped = program.mask(StepMask::Scoped);
        // Lanes of steps that are not candidates stay empty: a frame inherits
        // every lane.
        self.lanes.current.fill(0);
        self.lane_steps.clear();
        for (word, &candidates) in candidates.iter().enumerate() {
            let mut pending = candidates & scoped[word];
            while pending != 0 {
                self.lane_steps
                    .push(word * 64 + pending.trailing_zeros() as usize);
                pending &= pending - 1;
            }
        }

        for index in 0..self.lane_steps.len() {
            let step = self.lane_steps[index];
            let lane = program.plan(step).lane as usize;
            self.resolve_lane(program, step, depth, self.lanes.log.len(), lane, frames);
            if !bits::any(&self.lanes.current[self.lanes.range(lane)]) {
                candidates[step / 64] &= !(1 << (step % 64));
            }
        }
    }

    /// Write into `target` of the current lanes the lane of `step` for an
    /// element at `depth` opened when the lane log was `mark` long.
    ///
    /// `>` reads the parent's lane and `+`/`~` an earlier sibling's, which
    /// was computed at the same position, so both walk back to the step that
    /// starts the run: an entry step, whose lane comes from the scope stack,
    /// or a ` ` step, whose lane is what the element's ancestors inherited.
    fn resolve_lane(
        &mut self,
        program: &Program<'_>,
        mut step: usize,
        mut depth: usize,
        mut mark: usize,
        target: usize,
        frames: &[FrameMeta],
    ) {
        let range = self.lanes.range(target);
        loop {
            let plan = program.plan(step);
            match plan.lane_source {
                LaneSource::Child => {
                    // The parent matched the previous step, so it has a frame.
                    let index = frames.partition_point(|frame| (frame.depth as usize) < depth - 1);
                    if frames
                        .get(index)
                        .is_none_or(|frame| frame.depth as usize != depth - 1)
                    {
                        self.lanes.current[range].fill(0);
                        return;
                    }
                    mark = frames[index].log_mark as usize;
                    depth -= 1;
                    step -= 1;
                }
                LaneSource::Adjacent | LaneSource::Sibling => step -= 1,
                LaneSource::EntryDescendant | LaneSource::EntryChild => {
                    let entry_child = plan.lane_source == LaneSource::EntryChild;
                    let scope_section = program
                        .section_parent(plan.section)
                        .expect("nested section has a parent");
                    let lanes = &mut self.lanes.current[range];
                    lanes.fill(0);
                    for (slot, scope) in self.stack.iter().enumerate() {
                        let scope_depth = scope.depth as usize;
                        if scope.section == scope_section
                            && scope_depth < depth
                            && (!entry_child || scope_depth == depth - 1)
                        {
                            bits::set(lanes, slot);
                        }
                    }
                    return;
                }
                LaneSource::Descendant => {
                    let previous = program.plan(step - 1).lane as usize;
                    self.lanes.inherited_at(previous, mark, target);
                    return;
                }
                LaneSource::Never => {
                    self.lanes.current[range].fill(0);
                    return;
                }
            }
        }
    }

    /// Rows of the scopes an element saved by `save`'s section is listed
    /// under: the union of the lanes of the section's save steps among
    /// `candidates`. For a `First` section, scopes it already matched in are
    /// skipped, and the others are claimed.
    pub(super) fn parents(
        &mut self,
        program: &Program<'_>,
        save: StepPlan,
        candidates: &[u64],
    ) -> &[ElementId] {
        let section = save.section;
        self.slot_scratch.clear();
        self.slot_scratch.resize(self.lanes.words, 0);
        let saves = program.section_mask(section, SectionMask::Save);
        for (word, &candidates) in candidates.iter().enumerate() {
            let mut pending = candidates & saves[word];
            while pending != 0 {
                let step = word * 64 + pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let lane = self.lanes.range(program.plan(step).lane as usize);
                bits::or_assign(&mut self.slot_scratch, &self.lanes.current[lane]);
            }
        }

        let child = save.child as usize;
        let claim_words = program.claim_words();
        self.parent_scratch.clear();
        for slot in bits::iter_ones(&self.slot_scratch) {
            if save.first {
                let claims = &mut self.claims[slot * claim_words..][..claim_words];
                if bits::contains(claims, child) {
                    continue;
                }
                bits::set(claims, child);
            }
            self.parent_scratch.push(self.stack[slot].row);
        }
        &self.parent_scratch
    }

    /// Open a scope for `row`, an element at `depth` saved by `section`.
    pub(super) fn push(
        &mut self,
        program: &Program<'_>,
        section: SectionIndex,
        row: ElementId,
        depth: usize,
    ) {
        self.stack.push(Scope {
            depth: depth as u32,
            section,
            row,
        });
        self.claims
            .resize(self.claims.len() + program.claim_words(), 0);
        self.lanes.ensure_slots(self.stack.len());
        self.open[section.index()] += 1;
        if self.open[section.index()] == 1 {
            self.refresh_entry_descendant(program);
        }
    }

    /// Close the scopes of the element at `depth`.
    #[inline(never)]
    pub(super) fn pop_at(&mut self, program: &Program<'_>, depth: usize) {
        let mut refresh = false;
        while self
            .stack
            .last()
            .is_some_and(|scope| scope.depth as usize == depth)
        {
            let scope = self.stack.pop().unwrap();
            self.claims
                .truncate(self.stack.len() * program.claim_words());
            let open = &mut self.open[scope.section.index()];
            *open -= 1;
            refresh |= *open == 0;
        }
        if refresh {
            self.refresh_entry_descendant(program);
        }
    }

    fn refresh_entry_descendant(&mut self, program: &Program<'_>) {
        let words = self.entry_descendant.len();
        for word in 0..words {
            let mut entries = 0;
            for (section, &open) in self.open.iter().enumerate() {
                if open > 0 {
                    let section = SectionIndex(section as u32);
                    entries |=
                        program.section_mask(section, SectionMask::ChildEntryDescendant)[word];
                }
            }
            self.entry_descendant[word] = entries;
        }
    }

    /// Whether every open scope's nested sections are `First` sections that
    /// already matched in it, so no later element is saved under any scope.
    pub(super) fn finished(&self, program: &Program<'_>) -> bool {
        let claim_words = program.claim_words();
        self.stack.iter().enumerate().all(|(slot, scope)| {
            let claims = &self.claims[slot * claim_words..][..claim_words];
            program
                .children(scope.section)
                .iter()
                .enumerate()
                .all(|(child, &section)| {
                    program.section_kind(section) == SelectionKind::First
                        && bits::contains(claims, child)
                })
        })
    }
}
