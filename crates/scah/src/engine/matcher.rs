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
//! element saved by its parent section. Which scope a partial match belongs
//! to is tracked by [`Scopes`](super::scopes::Scopes), outside the step
//! bits: `prepare` asks it which nested entry steps open scopes enable,
//! `open` asks it to anchor scoped candidates, and `save` asks it which
//! scopes a match is listed under.

// Word loops index several rows at once and run `0..self.words()`, which is
// a constant for fixed-width matchers, so they unroll into register code.
#![allow(clippy::needless_range_loop)]

use std::ops::Range;

use scah_html::{AttributeMask, TagId};
use scah_query_ir::program::bits;
use scah_query_ir::{
    Program, SectionIndex, SectionMask, StepMask, StepPlan, StructuralMatchContext,
};
use smallvec::{SmallVec, smallvec};
use std::borrow::Cow;

use super::scopes::Scopes;
use crate::XHtmlElement;
use crate::output::Output;
use scah_store::ElementId;

/// Store writes made for one opened element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SaveHit {
    pub element_id: ElementId,
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
const SCRATCH: usize = 2;
const MASK_ROWS: usize = 3;

// Rows of each frame in `Matcher::frames`.
const MATCHED: usize = 0;
const INHERITED: usize = 1;
const PREV_SIBLING: usize = 2;
const ANY_SIBLING: usize = 3;
const FRAME_ROWS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub(super) struct FrameMeta {
    /// Depth of the element the frame belongs to; 0 for the document.
    pub(super) depth: u32,
    /// Length of the scopes' lane log when the frame was pushed.
    pub(super) log_mark: u32,
}

/// How a matcher holds its program: owned when compiled for one parse,
/// borrowed when reused. A type parameter rather than a `Cow`, so reading
/// the program in the per-tag hot path never branches on ownership.
pub(crate) trait ProgramRef<'q> {
    fn program(&self) -> &Program<'q>;
}

impl<'q> ProgramRef<'q> for Program<'q> {
    #[inline(always)]
    fn program(&self) -> &Program<'q> {
        self
    }
}

impl<'q> ProgramRef<'q> for &'q Program<'q> {
    #[inline(always)]
    fn program(&self) -> &Program<'q> {
        self
    }
}

pub(crate) struct Matcher<'q, P: ProgramRef<'q>, const W: usize> {
    store: P,
    marker: std::marker::PhantomData<&'q ()>,
    words: usize,
    siblings: bool,
    has_universal: bool,
    /// Open elements.
    depth: usize,

    /// The mutable rows; see the row constants. Everything immutable lives
    /// in the program.
    masks: Box<[u64]>,
    /// Which `First` root sections already saved their element.
    roots_claimed: Box<[bool]>,
    claimed_roots: usize,

    // Stacks and scratch space start inline, so shallow documents and parses
    // that stop early do not allocate for them.
    /// `FRAME_ROWS * words` words per frame. Frame 0 is the document.
    frames: SmallVec<[u64; 32]>,
    frame_meta: SmallVec<[FrameMeta; 8]>,
    scopes: Scopes,
}

impl<'q, P: ProgramRef<'q>, const W: usize> Matcher<'q, P, W> {
    pub(crate) fn new(store: P) -> Self {
        let program = store.program();
        let words = program.words();
        debug_assert!(W == 0 || W == words, "fixed width must match the program");
        let section_count = program.section_count();
        let mut masks = vec![0_u64; MASK_ROWS * words];
        masks[LIVE * words..(LIVE + 1) * words].fill(u64::MAX);

        Self {
            words,
            siblings: program.features().has_siblings,
            has_universal: bits::any(program.mask(StepMask::Universal)),
            depth: 0,
            masks: masks.into_boxed_slice(),
            roots_claimed: vec![false; section_count].into_boxed_slice(),
            claimed_roots: 0,
            frames: smallvec![0; FRAME_ROWS * words],
            frame_meta: smallvec![FrameMeta {
                depth: 0,
                log_mark: 0,
            }],
            scopes: Scopes::new(program),
            store,
            marker: std::marker::PhantomData,
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

    /// Word `word` of a program step mask, at the matcher's width.
    #[inline(always)]
    fn step_word(&self, mask: StepMask, word: usize) -> u64 {
        self.store.program().mask_word(mask, word, self.words())
    }

    /// Whether any candidate is in `mask`.
    #[inline(always)]
    fn candidates_in(&self, mask: StepMask) -> bool {
        let words = self.words();
        (0..words)
            .any(|word| self.masks[CANDIDATES * words + word] & self.step_word(mask, word) != 0)
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
            log_mark: self.scopes.log_mark(),
        });
    }

    fn pop_frame(&mut self) {
        let frame = self.top();
        debug_assert!(frame > 0, "the document frame is never popped");
        self.frames.truncate(frame * FRAME_ROWS * self.words());
        let meta = self.frame_meta.pop().unwrap();
        self.scopes.undo(meta.log_mark);
    }

    /// Select the steps an element named `name` (resolved to `tag`) could
    /// match at the current position. Returns whether there are any.
    #[inline(always)]
    pub(crate) fn prepare(&mut self, tag: TagId, name: &str) -> bool {
        let words = self.words();
        let named = self.store.program().tag_mask(tag, name);
        if named.is_none() && !self.has_universal {
            self.masks[CANDIDATES * words..(CANDIDATES + 1) * words].fill(0);
            return false;
        }

        let parent = self.depth;
        let top = self.top();
        let parent_frame = self.frame_meta[top].depth as usize == parent;
        let siblings = self.siblings && parent_frame;
        let scoped = !self.scopes.is_empty();
        let program = self.store.program();
        let mask = |mask: StepMask, word: usize| program.mask_word(mask, word, words);
        let frame = top * FRAME_ROWS * words;

        // One pass over the words; `carry_*` move bit 63 into the next word.
        let (mut carry_inherited, mut carry_matched) = (0, 0);
        let (mut carry_prev, mut carry_any) = (0, 0);
        let mut any = 0;
        for word in 0..words {
            let inherited = self.frames[frame + INHERITED * words + word];
            let mut candidates = mask(StepMask::RootEntryDescendant, word)
                | (((inherited << 1) | carry_inherited) & mask(StepMask::DescendantIn, word));
            carry_inherited = inherited >> 63;
            if parent == 0 {
                candidates |= mask(StepMask::RootEntryChild, word);
            }
            if parent_frame {
                let matched = self.frames[frame + MATCHED * words + word];
                candidates |= ((matched << 1) | carry_matched) & mask(StepMask::ChildIn, word);
                carry_matched = matched >> 63;
            }
            if siblings {
                let prev = self.frames[frame + PREV_SIBLING * words + word];
                let all = self.frames[frame + ANY_SIBLING * words + word];
                candidates |= ((prev << 1) | carry_prev) & mask(StepMask::AdjacentIn, word);
                candidates |= ((all << 1) | carry_any) & mask(StepMask::SiblingIn, word);
                carry_prev = prev >> 63;
                carry_any = all >> 63;
            }
            if scoped {
                candidates |= self.scopes.entries(program, parent, word, words);
            }
            let names = named.map_or(0, |named| named[word]);
            candidates &= self.masks[LIVE * words + word]
                & (names | mask(StepMask::Universal, word))
                & !(inherited & mask(StepMask::RedundantIfInherited, word));
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
                mask = mask.union(self.store.program().step_interest(step));
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
        out: &mut Output<'html, 'q>,
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
        if self.candidates_in(StepMask::Filter) {
            self.filter(element, structural, out);
        }

        let top = self.top();
        let scoped = self.scopes.has_lanes() && self.candidates_in(StepMask::Scoped);
        if scoped {
            let words = self.words();
            self.scopes.anchor(
                self.store.program(),
                &mut self.masks[CANDIDATES * words..(CANDIDATES + 1) * words],
                parent + 1,
                &self.frame_meta,
            );
        }

        // A frame is needed when a child or sibling reads one of these
        // matches, or when they add to the inherited set (or its lanes).
        let words = self.words();
        let mut needed = 0;
        let mut read = 0;
        for word in 0..words {
            let candidates = self.masks[CANDIDATES * words + word];
            let child_read = candidates & self.step_word(StepMask::ChildRead, word);
            let descendant_read = candidates & self.step_word(StepMask::DescendantRead, word);
            let inherited = self.frames[self.frame_word(top, INHERITED, word)];
            self.masks[SCRATCH * words + word] = child_read | descendant_read;
            read |= child_read | descendant_read;
            needed |= child_read | (descendant_read & !inherited);
        }
        let push = needed != 0 || (scoped && read != 0);
        if push {
            self.push_frame(self.depth, true);
            if scoped {
                self.scopes.inherit_current();
            }
        }

        if self.candidates_in(StepMask::Save) {
            self.save(element, out, hits);
        }
    }

    /// Drop candidates whose predicate rejects `element`.
    #[inline(never)]
    fn filter<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        structural: Option<&StructuralMatchContext<'q>>,
        out: &mut Output<'html, 'q>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let filter = self.store.program().mask(StepMask::Filter);
        for word in 0..words {
            let mut pending = self.masks[CANDIDATES * words + word] & filter[word];
            while pending != 0 {
                let bit = pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let step = word * 64 + bit;
                if !self
                    .store
                    .program()
                    .predicate(step)
                    .matches_named_element_with_context(element, structural)
                {
                    self.masks[CANDIDATES * words + word] &= !(1 << bit);
                    crate::scah_trace!(
                        out.trace,
                        crate::debug::TraceEvent::StepRejected {
                            selector: self
                                .store
                                .program()
                                .section(self.store.program().step_section(step))
                                .source,
                            element: element.name,
                            depth: self.depth as u16,
                            step,
                        }
                    );
                }
            }
        }
        #[cfg(not(any(debug_assertions, test)))]
        let _ = out;
    }

    #[inline(always)]
    fn save<'html>(
        &mut self,
        element: &XHtmlElement<'html>,
        out: &mut Output<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let mut last_section = None;
        for word in 0..words {
            let mut pending =
                self.masks[CANDIDATES * words + word] & self.step_word(StepMask::Save, word);
            while pending != 0 {
                let step = word * 64 + pending.trailing_zeros() as usize;
                pending &= pending - 1;
                let info = self.store.program().plan(step);
                // Alternatives of one section save the element once.
                if last_section == Some(info.section) {
                    continue;
                }
                last_section = Some(info.section);
                if info.root {
                    self.save_root(info, element, out, hits);
                } else {
                    self.save_scoped(info, element, out, hits);
                }
            }
        }
    }

    #[inline(always)]
    fn save_root<'html>(
        &mut self,
        info: StepPlan,
        element: &XHtmlElement<'html>,
        out: &mut Output<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let section = info.section;
        if info.first {
            self.claim_root(section);
        }
        let spec = self.store.program().section(section);
        let row = out.store.add_row(section.0, spec.save.attributes, element);
        add_edge(out, None, row, (section, spec), element);
        hits.push(save_hit(row, spec));
        if info.has_children {
            self.scopes
                .push(self.store.program(), section, row, self.depth);
        }
    }

    #[inline(never)]
    fn claim_root(&mut self, section: SectionIndex) {
        debug_assert!(!self.roots_claimed[section.index()]);
        self.roots_claimed[section.index()] = true;
        self.claimed_roots += 1;
        let words = self.words();
        let steps = self
            .store
            .program()
            .section_mask(section, SectionMask::Steps);
        for word in 0..words {
            self.masks[LIVE * words + word] &= !steps[word];
        }
    }

    #[inline(never)]
    fn save_scoped<'html>(
        &mut self,
        info: StepPlan,
        element: &XHtmlElement<'html>,
        out: &mut Output<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        let words = self.words();
        let section = info.section;
        let program = self.store.program();
        let spec = program.section(section);
        let candidates = &self.masks[CANDIDATES * words..(CANDIDATES + 1) * words];

        // One row for the element, listed under every scope it is saved in.
        let mut row = None;
        self.scopes
            .for_each_parent(program, info, candidates, |parent| {
                let row = *row.get_or_insert_with(|| {
                    out.store.add_row(section.0, spec.save.attributes, element)
                });
                add_edge(out, Some(parent), row, (section, spec), element);
            });
        let Some(row) = row else {
            return;
        };
        hits.push(save_hit(row, spec));
        if info.has_children {
            self.scopes
                .push(self.store.program(), section, row, self.depth);
        }
    }

    /// Close the innermost open element.
    #[inline(always)]
    pub(crate) fn close(&mut self) {
        debug_assert!(self.depth > 0, "close without a matching open");
        let depth = self.depth;
        self.depth -= 1;

        if !self.scopes.is_empty() {
            self.scopes.pop_at(self.store.program(), depth);
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

    /// Whether no later element can be saved: every root section is a
    /// claimed `First`, and every open scope's nested sections are claimed
    /// `First` sections too.
    pub(crate) fn finished(&self) -> bool {
        if !self.store.program().features().all_roots_first
            || self.claimed_roots != self.store.program().root_sections().len()
        {
            return false;
        }
        self.scopes.finished(self.store.program())
    }
}

/// A matcher specialized for the program's mask width.
///
/// Nearly every program has at most 64 steps, so its masks are single words
/// and the fixed-width matcher keeps them in registers.
pub(crate) enum AnyMatcher<'q> {
    Single(Matcher<'q, Program<'q>, 1>),
    Multi(Matcher<'q, Program<'q>, 0>),
    SingleBorrowed(Matcher<'q, &'q Program<'q>, 1>),
    MultiBorrowed(Matcher<'q, &'q Program<'q>, 0>),
}

macro_rules! dispatch {
    ($self:expr, $matcher:ident => $call:expr) => {
        match $self {
            AnyMatcher::Single($matcher) => $call,
            AnyMatcher::Multi($matcher) => $call,
            AnyMatcher::SingleBorrowed($matcher) => $call,
            AnyMatcher::MultiBorrowed($matcher) => $call,
        }
    };
}

impl<'q> AnyMatcher<'q> {
    pub(crate) fn new(program: Cow<'q, Program<'q>>) -> Self {
        match program {
            Cow::Owned(program) if program.words() == 1 => Self::Single(Matcher::new(program)),
            Cow::Owned(program) => Self::Multi(Matcher::new(program)),
            Cow::Borrowed(program) if program.words() == 1 => {
                Self::SingleBorrowed(Matcher::new(program))
            }
            Cow::Borrowed(program) => Self::MultiBorrowed(Matcher::new(program)),
        }
    }

    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        dispatch!(self, matcher => matcher.depth())
    }

    #[inline(always)]
    pub(crate) fn prepare(&mut self, tag: TagId, name: &str) -> bool {
        dispatch!(self, matcher => matcher.prepare(tag, name))
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
        out: &mut Output<'html, 'q>,
        hits: &mut Vec<SaveHit>,
    ) where
        'q: 'html,
    {
        dispatch!(self, matcher => matcher.open(element, structural, out, hits))
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
        save_inner_html: spec.save.inner_html,
        save_raw_text: spec.save.raw_text,
        save_text: spec.save.text,
    }
}

/// List `row` under `parent` (the document when `None`).
#[inline(always)]
fn add_edge<'html, 'q: 'html>(
    out: &mut Output<'html, 'q>,
    parent: Option<ElementId>,
    row: ElementId,
    section: (SectionIndex, &'q scah_query_ir::QuerySection<'q>),
    element: &XHtmlElement<'html>,
) {
    out.store.add_edge(parent, row);
    let (_index, _spec) = section;
    let _ = element;
    crate::scah_trace!(
        out.trace,
        crate::debug::TraceEvent::ElementSaved {
            section: _index.index(),
            selector: _spec.source,
            element: element.name,
            element_id: row,
            parent_id: parent.unwrap_or_default(),
            save_inner_html: _spec.save.inner_html,
            save_raw_text: _spec.save.raw_text,
            save_text: _spec.save.text,
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Query, QuerySpec, Save, Store};
    use pretty_assertions::assert_eq;
    use scah_store::StoreBuilder;

    /// Every tag name, id and class the tests use, space-separated. The
    /// store keeps strings as spans of the parsed document, so the driver's
    /// elements borrow from this one.
    static VOCABULARY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let mut words = String::from(
            " a b c class d d1 d2 d3 div e h1 h2 id li one p row s1 s2 section span two u u1 u2 ul w x y z ",
        );
        for index in 0..100 {
            words.push_str(&format!("d{index} ad{index} "));
        }
        words
    });

    /// `token` as a slice of [`VOCABULARY`].
    fn word(token: &str) -> &'static str {
        let vocabulary: &'static str = &VOCABULARY;
        let start = vocabulary
            .find(&format!(" {token} "))
            .unwrap_or_else(|| panic!("add {token:?} to the test vocabulary"))
            + 1;
        &vocabulary[start..start + token.len()]
    }

    /// Feeds synthetic open/close events. Elements are written as
    /// `name#id.class`.
    struct Driver<'q> {
        matcher: AnyMatcher<'q>,
        out: Output<'q, 'q>,
        hits: Vec<SaveHit>,
    }

    impl<'q> Driver<'q> {
        fn new<Q: QuerySpec<'q>>(queries: &'q [Q]) -> Self {
            let program = Program::compile(queries);
            let mut store = StoreBuilder::default();
            store.set_html(&VOCABULARY);
            let out = Output::new(store, &program, 0);
            Self {
                matcher: AnyMatcher::new(Cow::Owned(program)),
                out,
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
            let (id, class) = (id.map(word), class.map(word));
            // Like the parser, list `class` and `id` first among the
            // attributes, which is where saved rows read them from.
            let attributes: Vec<_> = [("class", class), ("id", id)]
                .into_iter()
                .filter(|(_, value)| value.is_some())
                .map(|(key, value)| Attribute {
                    key: word(key),
                    value,
                })
                .collect();
            let element = XHtmlElement {
                name: word(name),
                id,
                class,
                attributes: Box::leak(attributes.into_boxed_slice()),
            };
            self.matcher.prepare(TagId::of(name), name);
            self.matcher
                .open(&element, None, &mut self.out, &mut self.hits);
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
        fn render(self) -> String {
            fn label(element: crate::ElementRef<'_, '_, '_>) -> String {
                element.id().unwrap_or(element.name()).to_string()
            }
            fn rows(store: &Store<'_, '_>, ids: &[ElementId], out: &mut String) {
                let items: Vec<String> = ids
                    .iter()
                    .map(|&id| {
                        let mut item = label(store.element(id).expect("saved row"));
                        let mut children = Vec::new();
                        let mut index = 0;
                        while let Some(selector) = store.nested_selector(id, index) {
                            if let Some(nested) = store.nested_results(id, index) {
                                let mut list = String::new();
                                rows(store, nested, &mut list);
                                children.push(format!("{selector}[{list}]"));
                            }
                            index += 1;
                        }
                        if !children.is_empty() {
                            item.push_str(&format!("{{{}}}", children.join(" ")));
                        }
                        item
                    })
                    .collect();
                out.push_str(&items.join(", "));
            }
            let store = self.out.store.finish();
            let mut out = Vec::new();
            let mut index = 0;
            while let Some(selector) = store.query_selector(index) {
                if let Some(ids) = store.query_results(index) {
                    let mut list = String::new();
                    rows(&store, ids, &mut list);
                    out.push(format!("{selector}[{list}]"));
                }
                index += 1;
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
        matcher.scopes.lane_words()
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
            assert_eq!(matcher.scopes.log_mark(), 0, "{selector}");
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
        assert_eq!(driver.out.store.len(), 1);
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
        assert!(driver.matcher.prepare(TagId::A, "a"));
        let mask = driver.matcher.attribute_mask();
        assert_eq!(mask.keys, 0b1, "only `href` is inspected");
        assert_eq!(mask.flags & AttributeMask::ALL, 0);

        assert!(driver.matcher.prepare(TagId::P, "p"));
        assert_ne!(
            driver.matcher.attribute_mask().flags & AttributeMask::ALL,
            0
        );

        assert!(!driver.matcher.prepare(TagId::SPAN, "span"));
        assert!(driver.matcher.attribute_mask().is_empty());
    }
}
