//! Bitset matcher over a compiled [`Program`].
//!
//! Selectors without `:has()` only look at an element's ancestors and earlier
//! siblings, so whether an element matches a selector prefix is fully decided
//! by what its parent and earlier siblings matched. The matcher keeps that
//! information as bitsets, one bit per program step:
//!
//! - `matched`: steps matched by the element itself (feeds `>`);
//! - `inherited`: `matched` of the element and all its ancestors (feeds ` `);
//! - `prev_sibling` / `any_sibling`: `matched` of the last closed child, and
//!   of every closed child, of the element (feed `+` and `~`).
//!
//! Opening an element shifts its parent's rows by one step, masks them by the
//! incoming combinator, and keeps the steps whose predicate accepts the
//! element. No per-match state is created, so the work per tag does not
//! depend on how many partial matches exist.
//!
//! ## Sparse frames
//!
//! A *frame* (one set of the rows above) only exists where it carries
//! information: for the document, for elements that matched a step a child or
//! sibling reads, for elements that add to the inherited set, and for
//! elements whose children left sibling state behind. Every other element
//! just bumps a depth counter. In particular a run of nested `div`s under
//! `div p` pushes one frame, not one per `div`: once an ancestor matched
//! `div`, the inner ones are not even candidates.
//!
//! ## Scopes
//!
//! A nested section (`.then(...)`) matches relative to each *scope*: an open
//! element saved by the parent section. A match is attached to every scope
//! that contains the start of its chain, which a plain bit cannot express. For
//! steps of nested sections, the matcher also tracks *lanes*: the set of open
//! scopes (by stack slot) that each partial match is anchored in. Lanes are
//! only computed for steps that matched.
//!
//! A lane depends only on where the element is. An entry step's lane comes
//! from the scope stack, a ` ` step's from the lanes its ancestors inherit,
//! and a `>`, `+` or `~` step copies the previous step's lane from its parent
//! or an earlier sibling, which shares the element's position. So only the
//! inherited lanes are stored: one live set, with a log of the bits each
//! frame added so popping it can take them out. Every other lane is
//! recomputed when it is read. Memory stays linear in the open scopes, even
//! when every one of thousands of nested scopes anchors a partial match.

// Word loops index several rows at once and run `0..self.words()`, which is
// a constant for fixed-width matchers, so they unroll into register code.
#![allow(clippy::needless_range_loop)]

use std::ops::Range;

use scah_query_ir::program::bits;
use scah_query_ir::{
    AttributeMask, Program, SectionIndex, SectionMask, SelectionKind, StepMask,
    StructuralMatchContext,
};
use smallvec::{SmallVec, smallvec};

use crate::XHtmlElement;
use crate::store::{ElementId, Store};

/// Store writes made for one opened element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SaveHit {
    pub element_id: ElementId,
    pub save_attributes: bool,
    pub save_inner_html: bool,
    pub save_raw_text: bool,
    pub save_text: bool,
}

impl SaveHit {
    /// True when closing the element must finalize deferred content ranges.
    #[inline]
    pub(crate) fn needs_close_finalization(&self) -> bool {
        self.save_inner_html || self.save_raw_text || self.save_text
    }
}

// Rows of `Matcher::masks`, `words` words each.
const LIVE: usize = 0;
const CANDIDATES: usize = 1;
const ENTRY_DESCENDANT: usize = 2;
const SCRATCH: usize = 3;
/// Steps read by a child or later sibling: those followed by `>`, `+`, `~`.
const CHILD_READ: usize = 4;
/// Steps read by descendants: those followed by ` `.
const DESCENDANT_READ: usize = 5;
/// Steps that only descendants read and that neither save nor carry lanes:
/// matching one again under an ancestor that already matched it adds nothing.
const REDUNDANT_IF_INHERITED: usize = 6;
/// Per section, two rows: the ` ` and `>` entry steps of its children.
const SECTION_ROWS: usize = 7;

// Rows of each frame in `Matcher::frames`.
const MATCHED: usize = 0;
const INHERITED: usize = 1;
const PREV_SIBLING: usize = 2;
const ANY_SIBLING: usize = 3;
const FRAME_ROWS: usize = 4;

/// How a step's lane is derived from earlier frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaneSource {
    /// First step of a nested section entered through ` ` from its scope.
    EntryDescendant,
    /// First step of a nested section entered through `>` from its scope.
    EntryChild,
    Descendant,
    Child,
    Adjacent,
    Sibling,
    Never,
}

/// What saving an element at a save step does.
#[derive(Debug, Clone, Copy)]
struct StepSave {
    section: SectionIndex,
    /// Position of the section in its parent's child list.
    child: u32,
    root: bool,
    first: bool,
    has_children: bool,
}

/// Per-step matcher data, read together.
#[derive(Debug, Clone, Copy)]
struct StepState {
    /// Lane index; `u32::MAX` for root-section steps.
    lane: u32,
    source: LaneSource,
    save: StepSave,
}

#[derive(Debug, Clone, Copy, Default)]
struct SectionState {
    open_scopes: u32,
    root_claimed: bool,
}

#[derive(Debug, Clone, Copy)]
struct FrameMeta {
    /// Depth of the element the frame belongs to; 0 for the document.
    depth: u32,
    /// Length of [`Lanes::log`] when the frame was pushed.
    log_mark: u32,
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

/// An open element saved by a section that has nested sections.
#[derive(Debug, Clone)]
struct Scope {
    depth: u32,
    section: SectionIndex,
    /// Store rows of the element for this section: one per parent scope row
    /// it was saved under. Nested matches are saved under each of them.
    rows: Range<u32>,
}

pub(crate) struct Matcher<'q, const W: usize> {
    program: Program<'q>,
    words: usize,
    siblings: bool,
    has_universal: bool,
    /// Open elements.
    depth: usize,

    /// Fixed rows, then two rows per section; see the row constants.
    masks: Box<[u64]>,
    steps: Box<[StepState]>,
    sections: Box<[SectionState]>,
    claimed_roots: usize,

    // Stacks and scratch space start inline, so shallow documents and parses
    // that stop early do not allocate for them.
    /// `FRAME_ROWS * words` words per frame. Frame 0 is the document.
    frames: SmallVec<[u64; 32]>,
    frame_meta: SmallVec<[FrameMeta; 8]>,
    lanes: Lanes,

    scopes: SmallVec<[Scope; 4]>,
    scope_rows: SmallVec<[ElementId; 8]>,
    /// `claim_words` words per scope: which `First` child sections already
    /// matched inside it, by position in the parent's child list.
    scope_claims: SmallVec<[u64; 4]>,
    claim_words: usize,

    saved_rows: SmallVec<[ElementId; 4]>,
    lane_steps: SmallVec<[usize; 8]>,
    slot_scratch: SmallVec<[u64; 2]>,
}

impl<'q, const W: usize> Matcher<'q, W> {
    pub(crate) fn new(program: Program<'q>) -> Self {
        let words = program.words();
        debug_assert!(W == 0 || W == words, "fixed width must match the program");
        let section_count = program.section_count();

        let mut masks = vec![0_u64; (SECTION_ROWS + 2 * section_count) * words];
        let row = |row: usize| row * words..(row + 1) * words;
        masks[row(LIVE)].fill(u64::MAX);
        // Bit `s` of a read row is set when step `s + 1` reads step `s`.
        let shifted_down = |mask: &[u64], target: &mut [u64]| {
            for word in 0..words {
                let next = mask.get(word + 1).copied().unwrap_or(0);
                target[word] |= (mask[word] >> 1) | (next << 63);
            }
        };
        for mask in [StepMask::ChildIn, StepMask::AdjacentIn, StepMask::SiblingIn] {
            shifted_down(program.mask(mask), &mut masks[row(CHILD_READ)]);
        }
        shifted_down(
            program.mask(StepMask::DescendantIn),
            &mut masks[row(DESCENDANT_READ)],
        );
        for word in 0..words {
            masks[REDUNDANT_IF_INHERITED * words + word] = masks[DESCENDANT_READ * words + word]
                & !masks[CHILD_READ * words + word]
                & !program.mask(StepMask::Save)[word]
                & !program.mask(StepMask::Scoped)[word];
        }

        let mut claim_words = 1;
        for section in 0..section_count {
            let section = SectionIndex(section as u32);
            claim_words = claim_words.max(bits::words_for(program.children(section).len()));
            let descendant = row(SECTION_ROWS + 2 * section.index());
            let child = row(SECTION_ROWS + 2 * section.index() + 1);
            for &nested in program.children(section) {
                bits::or_assign(
                    &mut masks[descendant.clone()],
                    program.section_mask(nested, SectionMask::EntryDescendant),
                );
                bits::or_assign(
                    &mut masks[child.clone()],
                    program.section_mask(nested, SectionMask::EntryChild),
                );
            }
        }

        let mut lane_count = 0;
        let steps = (0..program.step_count())
            .map(|step| {
                let section = program.step_section(step);
                let parent = program.section_parent(section);
                let in_mask = |mask: &[u64]| bits::contains(mask, step);
                let (lane, source) = if parent.is_none() {
                    (u32::MAX, LaneSource::Never)
                } else {
                    lane_count += 1;
                    let source =
                        if in_mask(program.section_mask(section, SectionMask::EntryDescendant)) {
                            LaneSource::EntryDescendant
                        } else if in_mask(program.section_mask(section, SectionMask::EntryChild)) {
                            LaneSource::EntryChild
                        } else if in_mask(program.mask(StepMask::DescendantIn)) {
                            LaneSource::Descendant
                        } else if in_mask(program.mask(StepMask::ChildIn)) {
                            LaneSource::Child
                        } else if in_mask(program.mask(StepMask::AdjacentIn)) {
                            LaneSource::Adjacent
                        } else if in_mask(program.mask(StepMask::SiblingIn)) {
                            LaneSource::Sibling
                        } else {
                            LaneSource::Never
                        };
                    (lane_count - 1, source)
                };
                StepState {
                    lane,
                    source,
                    save: StepSave {
                        section,
                        child: parent.map_or(0, |parent| {
                            program
                                .children(parent)
                                .iter()
                                .position(|&child| child == section)
                                .expect("nested section is a child of its parent")
                                as u32
                        }),
                        root: parent.is_none(),
                        first: program.section_kind(section) == SelectionKind::First,
                        has_children: program.has_children(section),
                    },
                }
            })
            .collect();

        Self {
            words,
            siblings: program.features().has_siblings,
            has_universal: bits::any(program.mask(StepMask::Universal)),
            depth: 0,
            masks: masks.into_boxed_slice(),
            steps,
            sections: vec![SectionState::default(); section_count].into_boxed_slice(),
            claimed_roots: 0,
            frames: smallvec![0; FRAME_ROWS * words],
            frame_meta: smallvec![FrameMeta {
                depth: 0,
                log_mark: 0,
            }],
            lanes: Lanes {
                steps: lane_count as usize,
                words: 1,
                live: smallvec![0; lane_count as usize],
                log: SmallVec::new(),
                current: smallvec![0; lane_count as usize],
            },
            scopes: SmallVec::new(),
            scope_rows: SmallVec::new(),
            scope_claims: SmallVec::new(),
            claim_words,
            saved_rows: SmallVec::new(),
            lane_steps: SmallVec::new(),
            slot_scratch: SmallVec::new(),
            program,
        }
    }

    /// Words per step mask: `W` when fixed at compile time.
    #[inline(always)]
    fn words(&self) -> usize {
        if W == 0 { self.words } else { W }
    }

    /// Open elements.
    #[cfg(test)]
    #[inline]
    pub(crate) fn depth(&self) -> usize {
        self.depth
    }

    #[inline(always)]
    fn mask(&self, row: usize) -> &[u64] {
        let words = self.words();
        &self.masks[row * words..(row + 1) * words]
    }

    #[inline(always)]
    fn frame_word(&self, frame: usize, row: usize, word: usize) -> usize {
        (frame * FRAME_ROWS + row) * self.words() + word
    }

    #[inline(always)]
    fn frame_row(&self, frame: usize, row: usize) -> Range<usize> {
        let start = self.frame_word(frame, row, 0);
        start..start + self.words()
    }

    /// Index of the innermost frame.
    #[inline(always)]
    fn top(&self) -> usize {
        self.frame_meta.len() - 1
    }

    /// The frame of the element at `depth`, if it has one.
    #[inline(always)]
    fn frame_at(&self, depth: usize) -> Option<usize> {
        let top = self.top();
        (self.frame_meta[top].depth as usize == depth).then_some(top)
    }

    /// Push a frame for the element at `depth` whose matches are the
    /// `SCRATCH` row (or nothing), inheriting from the innermost frame, which
    /// is its nearest ancestor with a frame.
    fn push_frame(&mut self, depth: usize, matched: bool) {
        let words = self.words();
        let top = self.top();
        let base = self.frames.len();
        self.frames.resize(base + FRAME_ROWS * words, 0);
        for word in 0..words {
            let own = if matched {
                self.masks[SCRATCH * words + word]
            } else {
                0
            };
            self.frames[base + MATCHED * words + word] = own;
            self.frames[base + INHERITED * words + word] =
                self.frames[self.frame_word(top, INHERITED, word)] | own;
        }
        self.frame_meta.push(FrameMeta {
            depth: depth as u32,
            log_mark: self.lanes.log.len() as u32,
        });
    }

    fn pop_frame(&mut self) {
        let frame = self.top();
        debug_assert!(frame > 0, "the document frame is never popped");
        self.frames.truncate(frame * FRAME_ROWS * self.words());
        let meta = self.frame_meta.pop().unwrap();
        self.lanes.undo(meta.log_mark);
    }

    /// Select the steps an element named `name` could match at the current
    /// position. Returns whether there are any.
    #[inline(always)]
    pub(crate) fn prepare(&mut self, name: &str) -> bool {
        let words = self.words();
        let named = self.program.name_mask(name);
        if named.is_none() && !self.has_universal {
            self.masks[CANDIDATES * words..(CANDIDATES + 1) * words].fill(0);
            return false;
        }

        let parent = self.depth;
        let top = self.top();
        let parent_frame = self.frame_meta[top].depth as usize == parent;
        let siblings = self.siblings && parent_frame;
        let scoped = !self.scopes.is_empty();
        let program = &self.program;
        let root_descendant = program.mask(StepMask::RootEntryDescendant);
        let root_child = program.mask(StepMask::RootEntryChild);
        let descendant_in = program.mask(StepMask::DescendantIn);
        let child_in = program.mask(StepMask::ChildIn);
        let universal = program.mask(StepMask::Universal);
        let frame = top * FRAME_ROWS * words;

        // One pass over the words; `carry_*` move bit 63 into the next word.
        let (mut carry_inherited, mut carry_matched) = (0, 0);
        let (mut carry_prev, mut carry_any) = (0, 0);
        let mut any = 0;
        for word in 0..words {
            let inherited = self.frames[frame + INHERITED * words + word];
            let mut candidates = root_descendant[word]
                | (((inherited << 1) | carry_inherited) & descendant_in[word]);
            carry_inherited = inherited >> 63;
            if parent == 0 {
                candidates |= root_child[word];
            }
            if parent_frame {
                let matched = self.frames[frame + MATCHED * words + word];
                candidates |= ((matched << 1) | carry_matched) & child_in[word];
                carry_matched = matched >> 63;
            }
            if siblings {
                let prev = self.frames[frame + PREV_SIBLING * words + word];
                let all = self.frames[frame + ANY_SIBLING * words + word];
                candidates |= ((prev << 1) | carry_prev) & program.mask(StepMask::AdjacentIn)[word];
                candidates |= ((all << 1) | carry_any) & program.mask(StepMask::SiblingIn)[word];
                carry_prev = prev >> 63;
                carry_any = all >> 63;
            }
            if scoped {
                candidates |= self.masks[ENTRY_DESCENDANT * words + word];
                for scope in self.scopes.iter().rev() {
                    if scope.depth as usize != parent {
                        break;
                    }
                    let row = SECTION_ROWS + 2 * scope.section.index() + 1;
                    candidates |= self.masks[row * words + word];
                }
            }
            let names = named.map_or(0, |named| named[word]);
            candidates &= self.masks[LIVE * words + word]
                & (names | universal[word])
                & !(inherited & self.masks[REDUNDANT_IF_INHERITED * words + word]);
            self.masks[CANDIDATES * words + word] = candidates;
            any |= candidates;
        }
        any != 0
    }

    /// Attributes the prepared candidates inspect, or every attribute when a
    /// candidate saves them.
    #[inline(always)]
    pub(crate) fn attribute_mask(&self) -> AttributeMask {
        let mut mask = AttributeMask::default();
        for (word, &candidates) in self.mask(CANDIDATES).iter().enumerate() {
            let mut pending = candidates;
            while pending != 0 {
                let step = word * 64 + pending.trailing_zeros() as usize;
                pending &= pending - 1;
                mask = mask.union(self.program.step_interest(step));
            }
        }
        mask
    }

    /// Open the prepared element and save its matches.
    #[inline(always)]
    pub(crate) fn open<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        structural: Option<&StructuralMatchContext<'q>>,
        store: &mut Store<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        hits.clear();
        let parent = self.depth;
        self.depth += 1;
        if !bits::any(self.mask(CANDIDATES)) {
            return;
        }
        if bits::intersects(self.mask(CANDIDATES), self.program.mask(StepMask::Filter)) {
            self.filter(element, structural, store);
        }

        let top = self.top();
        let scoped = self.lanes.steps > 0
            && bits::intersects(self.mask(CANDIDATES), self.program.mask(StepMask::Scoped));
        if scoped {
            self.compute_lanes(parent);
        }

        // A frame is needed when a child or sibling reads one of these
        // matches, or when they add to the inherited set (or its lanes).
        let words = self.words();
        let mut needed = 0;
        let mut read = 0;
        for word in 0..words {
            let candidates = self.masks[CANDIDATES * words + word];
            let child_read = candidates & self.masks[CHILD_READ * words + word];
            let descendant_read = candidates & self.masks[DESCENDANT_READ * words + word];
            let inherited = self.frames[self.frame_word(top, INHERITED, word)];
            self.masks[SCRATCH * words + word] = child_read | descendant_read;
            read |= child_read | descendant_read;
            needed |= child_read | (descendant_read & !inherited);
        }
        let push = needed != 0 || (scoped && read != 0);
        if push {
            self.push_frame(self.depth, true);
            if scoped {
                self.lanes.inherit_current();
            }
        }

        if bits::intersects(self.mask(CANDIDATES), self.program.mask(StepMask::Save)) {
            self.save(element, store, hits);
        }
    }

    /// Drop candidates whose predicate rejects `element`.
    #[inline(never)]
    fn filter<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        structural: Option<&StructuralMatchContext<'q>>,
        store: &mut Store<'html, 'q>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let filter = self.program.mask(StepMask::Filter);
        for word in 0..words {
            let mut pending = self.masks[CANDIDATES * words + word] & filter[word];
            while pending != 0 {
                let bit = pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let step = word * 64 + bit;
                if !self
                    .program
                    .predicate(step)
                    .matches_named_element_with_context(element, structural)
                {
                    self.masks[CANDIDATES * words + word] &= !(1 << bit);
                    crate::scah_trace!(
                        store,
                        crate::debug::TraceEvent::StepRejected {
                            selector: self.program.section(self.program.step_section(step)).source,
                            element: element.name,
                            depth: self.depth as u16,
                            step,
                        }
                    );
                }
            }
        }
        #[cfg(not(any(debug_assertions, test)))]
        let _ = store;
    }

    /// Fill lanes for the scoped candidates of the element being opened, and
    /// drop candidates no scope can anchor.
    #[inline(never)]
    fn compute_lanes(&mut self, parent: usize) {
        let words = self.words();
        let scoped = self.program.mask(StepMask::Scoped);
        // Lanes of steps that are not candidates stay empty: a frame inherits
        // every lane.
        self.lanes.current.fill(0);
        self.lane_steps.clear();
        for word in 0..words {
            let mut pending = self.masks[CANDIDATES * words + word] & scoped[word];
            while pending != 0 {
                self.lane_steps
                    .push(word * 64 + pending.trailing_zeros() as usize);
                pending &= pending - 1;
            }
        }

        for index in 0..self.lane_steps.len() {
            let step = self.lane_steps[index];
            let lane = self.steps[step].lane as usize;
            self.resolve_lane(step, parent + 1, self.lanes.log.len(), lane);
            if !bits::any(&self.lanes.current[self.lanes.range(lane)]) {
                self.masks[CANDIDATES * words + step / 64] &= !(1 << (step % 64));
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
    fn resolve_lane(&mut self, mut step: usize, mut depth: usize, mut mark: usize, target: usize) {
        let range = self.lanes.range(target);
        loop {
            let state = self.steps[step];
            match state.source {
                LaneSource::Child => {
                    // The parent matched the previous step, so it has a frame.
                    let frames = &self.frame_meta;
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
                    let entry_child = state.source == LaneSource::EntryChild;
                    let scope_section = self
                        .program
                        .section_parent(state.save.section)
                        .expect("nested section has a parent");
                    let lanes = &mut self.lanes.current[range];
                    lanes.fill(0);
                    for (slot, scope) in self.scopes.iter().enumerate() {
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
                    let previous = self.steps[step - 1].lane as usize;
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

    #[inline(always)]
    fn save<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        store: &mut Store<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let mut last_section = None;
        for word in 0..words {
            let mut pending =
                self.masks[CANDIDATES * words + word] & self.program.mask(StepMask::Save)[word];
            while pending != 0 {
                let step = word * 64 + pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let info = self.steps[step].save;
                // Alternatives of one section save the element once.
                if last_section == Some(info.section) {
                    continue;
                }
                last_section = Some(info.section);
                if info.root {
                    self.save_root(info, element, store, hits);
                } else {
                    self.save_scoped(info, element, store, hits);
                }
            }
        }
    }

    #[inline(always)]
    fn save_root<'html>(
        &mut self,
        info: StepSave,
        element: &XHtmlElement<'html>,
        store: &mut Store<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let section = info.section;
        if info.first {
            self.claim_root(section);
        }
        let spec = self.program.section(section);
        let element_id = push_row(store, ElementId::default(), (section, spec), element);
        hits.push(save_hit(element_id, spec));
        if info.has_children {
            self.open_root_scope(section, element_id);
        }
    }

    #[inline(never)]
    fn claim_root(&mut self, section: SectionIndex) {
        let state = &mut self.sections[section.index()];
        debug_assert!(!state.root_claimed);
        state.root_claimed = true;
        self.claimed_roots += 1;
        let words = self.words();
        let steps = self.program.section_mask(section, SectionMask::Steps);
        for word in 0..words {
            self.masks[LIVE * words + word] &= !steps[word];
        }
    }

    #[inline(never)]
    fn open_root_scope(&mut self, section: SectionIndex, element_id: ElementId) {
        self.saved_rows.clear();
        self.saved_rows.push(element_id);
        self.open_scope(section);
    }

    #[inline(never)]
    fn save_scoped<'html>(
        &mut self,
        info: StepSave,
        element: &XHtmlElement<'html>,
        store: &mut Store<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let section = info.section;
        let spec = self.program.section(section);

        // Union the anchors of every alternative that matched.
        self.slot_scratch.clear();
        self.slot_scratch.resize(self.lanes.words, 0);
        let saves = self.program.section_mask(section, SectionMask::Save);
        for word in 0..words {
            let mut pending = self.masks[CANDIDATES * words + word] & saves[word];
            while pending != 0 {
                let step = word * 64 + pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let lane = self.lanes.range(self.steps[step].lane as usize);
                bits::or_assign(&mut self.slot_scratch, &self.lanes.current[lane]);
            }
        }

        self.saved_rows.clear();
        let child = info.child as usize;
        for slot in bits::iter_ones(&self.slot_scratch) {
            if info.first {
                let claims = &mut self.scope_claims[slot * self.claim_words..][..self.claim_words];
                if bits::contains(claims, child) {
                    continue;
                }
                bits::set(claims, child);
            }
            for row in self.scopes[slot].rows.clone() {
                let parent = self.scope_rows[row as usize];
                let element_id = push_row(store, parent, (section, spec), element);
                self.saved_rows.push(element_id);
                hits.push(save_hit(element_id, spec));
            }
        }
        if info.has_children && !self.saved_rows.is_empty() {
            self.open_scope(section);
        }
    }

    fn open_scope(&mut self, section: SectionIndex) {
        let start = self.scope_rows.len() as u32;
        self.scope_rows.extend_from_slice(&self.saved_rows);
        self.scopes.push(Scope {
            depth: self.depth as u32,
            section,
            rows: start..self.scope_rows.len() as u32,
        });
        self.scope_claims
            .resize(self.scope_claims.len() + self.claim_words, 0);
        self.lanes.ensure_slots(self.scopes.len());
        let state = &mut self.sections[section.index()];
        state.open_scopes += 1;
        if state.open_scopes == 1 {
            self.refresh_entry_descendant();
        }
    }

    fn refresh_entry_descendant(&mut self) {
        let words = self.words();
        for word in 0..words {
            let mut entries = 0;
            for (section, state) in self.sections.iter().enumerate() {
                if state.open_scopes > 0 {
                    entries |= self.masks[(SECTION_ROWS + 2 * section) * words + word];
                }
            }
            self.masks[ENTRY_DESCENDANT * words + word] = entries;
        }
    }

    /// Close the innermost open element.
    #[inline(always)]
    pub(crate) fn close(&mut self) {
        debug_assert!(self.depth > 0, "close without a matching open");
        let depth = self.depth;
        self.depth -= 1;

        if !self.scopes.is_empty() {
            self.close_scopes(depth);
        }

        let Some(frame) = self.frame_at(depth) else {
            // The element left nothing a later sibling reads.
            self.clear_prev_sibling(depth - 1);
            return;
        };
        if !self.siblings {
            self.pop_frame();
            return;
        }
        self.close_frame_with_siblings(frame, depth);
    }

    #[inline(always)]
    fn clear_prev_sibling(&mut self, parent_depth: usize) {
        if self.siblings
            && let Some(parent) = self.frame_at(parent_depth)
        {
            let row = self.frame_row(parent, PREV_SIBLING);
            self.frames[row].fill(0);
        }
    }

    /// Pop `frame` and record its matches as its parent's previous sibling,
    /// giving the parent a frame if it has none yet.
    #[inline(never)]
    fn close_frame_with_siblings(&mut self, frame: usize, depth: usize) {
        let words = self.words();
        let mut any = 0;
        for word in 0..words {
            let matched = self.frames[self.frame_word(frame, MATCHED, word)];
            self.masks[SCRATCH * words + word] = matched;
            any |= matched;
        }
        if any == 0 {
            // A frame that only held its children's sibling state.
            self.pop_frame();
            self.clear_prev_sibling(depth - 1);
            return;
        }
        self.pop_frame();

        // A later sibling's lanes are recomputed at its own position, which
        // this element shared, so only the matched steps carry over.
        let parent = match self.frame_at(depth - 1) {
            Some(parent) => parent,
            None => {
                self.push_frame(depth - 1, false);
                self.top()
            }
        };
        for word in 0..words {
            let matched = self.masks[SCRATCH * words + word];
            let prev = self.frame_word(parent, PREV_SIBLING, word);
            self.frames[prev] = matched;
            let all = self.frame_word(parent, ANY_SIBLING, word);
            self.frames[all] |= matched;
        }
    }

    #[inline(never)]
    fn close_scopes(&mut self, depth: usize) {
        let mut refresh = false;
        while self
            .scopes
            .last()
            .is_some_and(|scope| scope.depth as usize == depth)
        {
            let scope = self.scopes.pop().unwrap();
            self.scope_rows.truncate(scope.rows.start as usize);
            self.scope_claims
                .truncate(self.scopes.len() * self.claim_words);
            let state = &mut self.sections[scope.section.index()];
            state.open_scopes -= 1;
            refresh |= state.open_scopes == 0;
        }
        if refresh {
            self.refresh_entry_descendant();
        }
    }

    /// Whether no later element can be saved: every root section is a
    /// claimed `First`, and every open scope's nested sections are claimed
    /// `First` sections too.
    pub(crate) fn finished(&self) -> bool {
        if !self.program.features().all_roots_first
            || self.claimed_roots != self.program.root_sections().len()
        {
            return false;
        }
        self.scopes.iter().enumerate().all(|(slot, scope)| {
            let claims = &self.scope_claims[slot * self.claim_words..][..self.claim_words];
            self.program
                .children(scope.section)
                .iter()
                .enumerate()
                .all(|(child, &section)| {
                    self.program.section_kind(section) == SelectionKind::First
                        && bits::contains(claims, child)
                })
        })
    }
}

/// A matcher specialized for the program's mask width.
///
/// Nearly every program has at most 64 steps, so its masks are single words
/// and the fixed-width matcher keeps them in registers.
pub(crate) enum AnyMatcher<'q> {
    Single(Matcher<'q, 1>),
    Multi(Matcher<'q, 0>),
}

macro_rules! dispatch {
    ($self:expr, $matcher:ident => $call:expr) => {
        match $self {
            AnyMatcher::Single($matcher) => $call,
            AnyMatcher::Multi($matcher) => $call,
        }
    };
}

impl<'q> AnyMatcher<'q> {
    pub(crate) fn new(program: Program<'q>) -> Self {
        if program.words() == 1 {
            Self::Single(Matcher::new(program))
        } else {
            Self::Multi(Matcher::new(program))
        }
    }

    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        dispatch!(self, matcher => matcher.depth())
    }

    #[inline(always)]
    pub(crate) fn prepare(&mut self, name: &str) -> bool {
        dispatch!(self, matcher => matcher.prepare(name))
    }

    #[inline(always)]
    pub(crate) fn attribute_mask(&self) -> AttributeMask {
        dispatch!(self, matcher => matcher.attribute_mask())
    }

    #[inline(always)]
    pub(crate) fn open<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        structural: Option<&StructuralMatchContext<'q>>,
        store: &mut Store<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        dispatch!(self, matcher => matcher.open(element, structural, store, hits))
    }

    #[inline(always)]
    pub(crate) fn close(&mut self) {
        dispatch!(self, matcher => matcher.close())
    }

    #[inline(always)]
    pub(crate) fn finished(&self) -> bool {
        dispatch!(self, matcher => matcher.finished())
    }
}

#[inline(always)]
fn save_hit(element_id: ElementId, spec: &scah_query_ir::QuerySection<'_>) -> SaveHit {
    SaveHit {
        element_id,
        save_attributes: spec.save.attributes,
        save_inner_html: spec.save.inner_html,
        save_raw_text: spec.save.raw_text,
        save_text: spec.save.text,
    }
}

/// Save `element` for `section` under `parent` (the document when null).
fn push_row<'html, 'q: 'html>(
    store: &mut Store<'html, 'q>,
    parent: ElementId,
    section: (SectionIndex, &'q scah_query_ir::QuerySection<'q>),
    element: &XHtmlElement<'html>,
) -> ElementId {
    let (_index, spec) = section;
    let element_id = store.push(parent, spec, element.clone());
    crate::scah_trace!(
        store,
        crate::debug::TraceEvent::ElementSaved {
            section: _index.index(),
            selector: spec.source,
            element: element.name,
            element_id,
            parent_id: parent,
            save_inner_html: spec.save.inner_html,
            save_raw_text: spec.save.raw_text,
            save_text: spec.save.text,
        }
    );
    element_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::QueryId;
    use crate::{Query, QuerySpec, Save};
    use pretty_assertions::assert_eq;

    /// Feeds synthetic open/close events. Elements are written as
    /// `name#id.class`.
    struct Driver<'q> {
        matcher: AnyMatcher<'q>,
        store: Store<'q, 'q>,
        hits: Vec<SaveHit>,
    }

    impl<'q> Driver<'q> {
        fn new<Q: QuerySpec<'q>>(queries: &'q [Q]) -> Self {
            Self {
                matcher: AnyMatcher::new(Program::compile(queries)),
                store: Store::default(),
                hits: Vec::new(),
            }
        }

        fn open(&mut self, tag: &'q str) -> &mut Self {
            let (rest, class) = tag
                .split_once('.')
                .map_or((tag, None), |(l, r)| (l, Some(r)));
            let (name, id) = rest
                .split_once('#')
                .map_or((rest, None), |(l, r)| (l, Some(r)));
            let element = XHtmlElement {
                name,
                id,
                class,
                attributes: &[],
            };
            self.matcher.prepare(name);
            self.matcher
                .open(&element, None, &mut self.store, &mut self.hits);
            self
        }

        fn close(&mut self) -> &mut Self {
            self.matcher.close();
            self
        }

        fn void(&mut self, tag: &'q str) -> &mut Self {
            self.open(tag).close()
        }

        /// Parse a compact tree: `div(p a) span` opens `div`, its children
        /// `p` and `a`, closes `div`, then a void-like `span`.
        fn feed(&mut self, tree: &'q str) -> &mut Self {
            let bytes = tree.as_bytes();
            let mut index = 0;
            while index < bytes.len() {
                match bytes[index] {
                    b' ' => index += 1,
                    b')' => {
                        self.close();
                        index += 1;
                    }
                    _ => {
                        let end = tree[index..]
                            .find([' ', '(', ')'])
                            .map_or(tree.len(), |offset| index + offset);
                        let tag = &tree[index..end];
                        if bytes.get(end) == Some(&b'(') {
                            self.open(tag);
                            index = end + 1;
                        } else {
                            self.void(tag);
                            index = end;
                        }
                    }
                }
            }
            self
        }

        /// Results as `selector[elem{child[...]}, ...]`, elements by id.
        fn render(&self) -> String {
            fn label(element: &crate::Element<'_>) -> String {
                element.id.unwrap_or(element.name).to_string()
            }
            fn queries(store: &Store<'_, '_>, first: Option<QueryId>, out: &mut Vec<String>) {
                let mut query = first;
                while let Some(id) = query {
                    let node = &store.queries[id];
                    let mut items = Vec::new();
                    for element in store.elements.iter_from(node.elements.start()) {
                        let mut item = label(element);
                        if element.first_child_query.is_some() {
                            let mut children = Vec::new();
                            queries(store, element.first_child_query, &mut children);
                            item.push_str(&format!("{{{}}}", children.join(" ")));
                        }
                        items.push(item);
                    }
                    out.push(format!("{}[{}]", node.query, items.join(", ")));
                    query = node.next_sibling;
                }
            }
            let mut out = Vec::new();
            if !self.store.queries.is_empty() {
                queries(&self.store, Some(QueryId(0)), &mut out);
            }
            out.join(" ")
        }
    }

    fn run<'q, Q: QuerySpec<'q>>(queries: &'q [Q], tree: &'q str) -> String {
        let mut driver = Driver::new(queries);
        driver.feed(tree);
        assert_eq!(driver.matcher.depth(), 0, "unbalanced test tree");
        driver.render()
    }

    #[test]
    fn descendant_matches_once_per_element() {
        let queries = [Query::all("div a", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "div(div(a#x) p(a#y)) a#z"), "div a[x, y]");
    }

    #[test]
    fn child_requires_the_direct_parent() {
        let queries = [Query::all("div > a", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "div(a#x p(a#y)) a#z"), "div > a[x]");
    }

    #[test]
    fn top_level_child_entry_matches_depth_one() {
        let queries = [Query::all("> a", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "a#x div(a#y)"), "> a[x]");
    }

    #[test]
    fn sibling_combinators_follow_closed_siblings() {
        let queries = [
            Query::all("h1 + p", Save::none()).unwrap().build(),
            Query::all("h1 ~ p", Save::none()).unwrap().build(),
        ];
        assert_eq!(
            run(&queries, "h1 p#a p#b div(p#c) h2 p#d"),
            "h1 + p[a] h1 ~ p[a, b, d]"
        );
    }

    #[test]
    fn adjacency_ignores_descendants_of_the_previous_sibling() {
        let queries = [Query::all("h1 + p", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "h1(p#a) p#b"), "h1 + p[b]");
    }

    #[test]
    fn selector_list_saves_an_element_once() {
        let queries = [Query::all("a, .x", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "a#one.x span#two.x"), "a, .x[one, two]");
    }

    #[test]
    fn filters_reject_candidates() {
        let queries = [Query::all("div.x > a#y", Save::none()).unwrap().build()];
        assert_eq!(run(&queries, "div.x(a#y a#z) div(a#y)"), "div.x > a#y[y]");
    }

    #[test]
    fn first_root_section_saves_one_element_and_finishes() {
        let queries = [Query::first("a", Save::none()).unwrap().build()];
        let mut driver = Driver::new(&queries);
        driver.feed("div(p");
        assert!(!driver.matcher.finished());
        driver.feed("a#x");
        assert!(driver.matcher.finished());
        driver.feed("a#y)");
        assert_eq!(driver.render(), "a[x]");
    }

    #[test]
    fn all_root_sections_never_finish() {
        let queries = [Query::all("a", Save::none()).unwrap().build()];
        let mut driver = Driver::new(&queries);
        driver.feed("a#x");
        assert!(!driver.matcher.finished());
    }

    #[test]
    fn nested_matches_attach_to_every_containing_scope() {
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .then(|div| {
                Ok([
                    div.all("ul > li", Save::none())?,
                    div.all("li p", Save::none())?,
                ])
            })
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "div#d1(ul(li#a(div#d2(ul(li#b) p#p))))"),
            "div[d1{ul > li[a, b] li p[p]}, d2{ul > li[b]}]"
        );
    }

    #[test]
    fn child_entry_anchors_only_the_parent_scope() {
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .all("> a", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "div#d1(a#x div#d2(a#y p(a#z)))"),
            "div[d1{> a[x]}, d2{> a[y]}]"
        );
    }

    #[test]
    fn child_entry_chains_keep_their_exact_anchor_set() {
        // li#b is inside d2 but its `ul` is a child of d1 only.
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .all("> ul li", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "div#d1(ul(li#a(div#d2(p(li#b)) ul(li#c))))"),
            "div[d1{> ul li[a, b, c]}, d2]"
        );
    }

    #[test]
    fn first_nested_section_is_claimed_per_scope() {
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .first("a", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "div#d1(div#d2(a#x a#y) a#z) div#d3(a#w)"),
            "div[d1{a[x]}, d2{a[x]}, d3{a[w]}]"
        );
    }

    #[test]
    fn nested_sections_fan_out_over_parent_copies() {
        let queries = [Query::all("section", Save::none())
            .unwrap()
            .all("div", Save::none())
            .unwrap()
            .all("a", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "section#s1(section#s2(div#d(a#x)))"),
            "section[s1{div[d{a[x]}]}, s2{div[d{a[x]}]}]"
        );
    }

    #[test]
    fn first_root_with_children_finishes_when_its_scope_closes() {
        let queries = [Query::first("div", Save::none())
            .unwrap()
            .all("a", Save::none())
            .unwrap()
            .build()];
        let mut driver = Driver::new(&queries);
        driver.feed("div#d1(a#x");
        assert!(!driver.matcher.finished());
        driver.feed("a#y)");
        assert!(driver.matcher.finished());
        driver.feed("div#d2(a#z)");
        assert_eq!(driver.render(), "div[d1{a[x, y]}]");
    }

    #[test]
    fn first_children_saturate_their_scope() {
        let queries = [Query::first("div", Save::none())
            .unwrap()
            .first("a", Save::none())
            .unwrap()
            .build()];
        let mut driver = Driver::new(&queries);
        driver.feed("div#d1(p");
        assert!(!driver.matcher.finished());
        driver.feed("a#x");
        assert!(driver.matcher.finished());
    }

    #[test]
    fn scoped_sibling_chains_use_sibling_lanes() {
        let queries = [Query::all("ul", Save::none())
            .unwrap()
            .all("> li + li", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "ul#u1(li#a li#b ul#u2(li#c li#d) li#e)"),
            "ul[u1{> li + li[b]}, u2{> li + li[d]}]"
        );
    }

    #[test]
    fn scoped_general_siblings_accumulate() {
        let queries = [Query::all("ul", Save::none())
            .unwrap()
            .all("> h1 ~ li", Save::none())
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "ul#u(li#a h1 li#b p li#c)"),
            "ul[u{> h1 ~ li[b, c]}]"
        );
    }

    /// Lane state the matcher holds: live lanes plus logged words.
    fn lane_words(driver: &Driver<'_>) -> usize {
        let AnyMatcher::Single(matcher) = &driver.matcher else {
            unreachable!("one-word program")
        };
        matcher.lanes.live.len() + matcher.lanes.log.len()
    }

    #[test]
    fn sibling_lanes_do_not_grow_with_siblings() {
        let queries = [Query::all("section", Save::none())
            .unwrap()
            .all("h1 + a", Save::none())
            .unwrap()
            .build()];
        let mut driver = Driver::new(&queries);
        driver.open("section");
        driver.feed("h1 span");
        let words = lane_words(&driver);
        for _ in 0..100 {
            driver.feed("h1 span");
        }
        assert_eq!(lane_words(&driver), words);
        driver.feed("h1 a#x").close();
        assert_eq!(driver.render(), "section[section{h1 + a[x]}]");
    }

    #[test]
    fn lanes_of_an_earlier_element_do_not_leak_into_a_frame() {
        // `p` pushes a frame (for `p > x`) right after the inner `a` computed
        // `a b` lanes for `s1` and `s2`. Slot 1 now holds `u1`, which has no
        // `a` inside, so `b` must not be saved under it.
        let queries = [Query::all("section", Save::none())
            .unwrap()
            .then(|section| {
                Ok([
                    section.all("a b", Save::none())?,
                    section.all("p > x", Save::none())?,
                ])
            })
            .unwrap()
            .build()];
        assert_eq!(
            run(&queries, "section#s1(a(section#s2(a) section#u1(p(b))))"),
            "section[s1{a b[b]}, s2, u1]"
        );
    }

    #[test]
    fn nested_scope_lanes_stay_linear() {
        // Every `div` is a scope, and every inner `div` starts a partial
        // match in each enclosing one: storing each frame's lanes would take
        // memory quadratic in the depth.
        for selector in ["div span", "div > span", "div + span"] {
            let queries = [Query::all("div", Save::none())
                .unwrap()
                .all(selector, Save::none())
                .unwrap()
                .build()];
            let mut driver = Driver::new(&queries);
            let depth = 1_000;
            for _ in 0..depth {
                driver.open("div");
            }
            let lanes = 2;
            let words = bits::words_for(depth);
            assert!(
                lane_words(&driver) <= 2 * lanes * words + lanes * depth,
                "{selector}: {} words",
                lane_words(&driver)
            );
            for _ in 0..depth {
                driver.close();
            }
            let AnyMatcher::Single(matcher) = &driver.matcher else {
                unreachable!("one-word program")
            };
            assert!(matcher.lanes.log.is_empty(), "{selector}");
        }
    }

    #[test]
    fn lanes_widen_past_sixty_four_open_scopes() {
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .all("> a", Save::none())
            .unwrap()
            .build()];
        let ids: Vec<String> = (0..70).map(|index| format!("d{index}")).collect();
        let mut tree = String::new();
        for id in &ids {
            tree.push_str(&format!("div#{id}(a#a{id} "));
        }
        tree.push_str(&")".repeat(ids.len()));
        let tree: &'static str = Box::leak(tree.into_boxed_str());
        let rendered = run(&queries, tree);
        let expected: Vec<String> = ids.iter().map(|id| format!("{id}{{> a[a{id}]}}")).collect();
        assert_eq!(rendered, format!("div[{}]", expected.join(", ")));
    }

    #[test]
    fn steps_past_sixty_four_carry_between_words() {
        let selector = vec!["div"; 66].join(" > ");
        let selector: &'static str = Box::leak(selector.into_boxed_str());
        let queries = [Query::all(selector, Save::none()).unwrap().build()];
        let tree = format!("{}{}", "div(".repeat(66), ")".repeat(66));
        let tree: &'static str = Box::leak(tree.into_boxed_str());
        let mut driver = Driver::new(&queries);
        driver.feed(tree);
        assert_eq!(driver.store.elements.len(), 1);
    }

    #[test]
    fn save_hits_report_content_flags() {
        let queries = [Query::all("a", Save::only_text()).unwrap().build()];
        let mut driver = Driver::new(&queries);
        driver.open("a");
        assert_eq!(driver.hits.len(), 1);
        assert!(driver.hits[0].save_text);
        assert!(driver.hits[0].needs_close_finalization());
        assert!(!driver.hits[0].save_inner_html);
    }

    #[test]
    fn attribute_masks_cover_candidates_and_saves() {
        let queries = [
            Query::all("div > a[href]", Save::name_only())
                .unwrap()
                .build(),
            Query::all("p", Save::none()).unwrap().build(),
        ];
        let mut driver = Driver::new(&queries);
        driver.open("div");
        assert!(driver.matcher.prepare("a"));
        let mask = driver.matcher.attribute_mask();
        assert_eq!(mask.keys, 0b1, "only `href` is inspected");
        assert_eq!(mask.flags & AttributeMask::ALL, 0);

        assert!(driver.matcher.prepare("p"));
        assert_ne!(
            driver.matcher.attribute_mask().flags & AttributeMask::ALL,
            0
        );

        assert!(!driver.matcher.prepare("span"));
        assert!(driver.matcher.attribute_mask().is_empty());
    }
}
