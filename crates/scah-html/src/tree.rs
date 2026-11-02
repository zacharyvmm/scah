//! The stack of open elements, and the HTML rules that close them: end
//! tags that close elements further down, open tags that close an open `p`,
//! `li`, `option` or table cell, and the end of the input.
//!
//! The stack indexes the open elements those implied closes look for, and the
//! scope barriers that stop them, and chains the open elements of each tag
//! name together, so an open or end tag decides what it closes in constant
//! time however deep the stack is.

use crate::TagId;
use crate::tag::{ScopeKind, TagFlags};

pub type DepthSize = u16;

/// Maximum real element depth.
pub const MAX_ELEMENT_DEPTH: DepthSize = DepthSize::MAX - 1;

/// An element would be nested deeper than [`MAX_ELEMENT_DEPTH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthExceeded;

#[derive(Debug, Clone, PartialEq)]
pub struct OpenElement<'html> {
    pub name: &'html str,
    /// `name` resolved once at the open tag; close tags compare ids.
    pub id: TagId,
    packed_flags: u32,
}

impl<'html> OpenElement<'html> {
    const PAYLOAD_SHIFT: u32 = 25;
    /// Bits left for the payload above the tag flags.
    pub const PAYLOAD_BITS: u32 = u32::BITS - Self::PAYLOAD_SHIFT;
    /// Set in `packed_flags`, below the payload, once the element is chained.
    const CHAINED: u32 = 1 << (Self::PAYLOAD_SHIFT - 1);

    #[inline]
    pub fn tag(&self) -> TagFlags {
        TagFlags::from_bits(self.packed_flags & (Self::CHAINED - 1))
    }

    /// The bits the caller pushed with the element. scah keeps the
    /// element's text-capture flags here, next to its tag flags.
    #[inline]
    pub fn payload(&self) -> u8 {
        (self.packed_flags >> Self::PAYLOAD_SHIFT) as u8
    }

    /// Whether a close tag named `name`, resolved to `id`, closes this
    /// element. Known tags compare ids; other names compare ignoring case.
    #[inline(always)]
    fn closed_by(&self, name: &str, id: TagId) -> bool {
        if id.is_known() || self.id.is_known() {
            self.id == id
        } else {
            self.name == name || self.name.eq_ignore_ascii_case(name)
        }
    }
}

/// End of a [`TagChains`] chain.
const NO_POSITION: DepthSize = DepthSize::MAX;
const _: () = assert!(NO_POSITION > MAX_ELEMENT_DEPTH);

/// Chains for names outside the known-tag table, picked by a hash of the name.
const UNKNOWN_CHAINS: usize = 256;

/// Barriers of the default scope, where every unknown name closes.
const DEFAULT_BARRIERS: TagFlags = match ScopeKind::Default.barriers() {
    Some(barriers) => barriers,
    None => unreachable!(),
};

/// Which [`TagChains`] chain an element is on: its tag id's, or for an
/// unknown name, the one its name hashes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chain {
    Known(TagId),
    Unknown(u8),
}

impl Chain {
    #[inline(always)]
    fn of(name: &str, id: TagId) -> Self {
        if id.is_known() {
            Self::Known(id)
        } else {
            // `| 0x20` folds ASCII case, so names that are equal ignoring
            // case hash alike. Other bytes it merges only share a chain.
            let hash = name.bytes().fold(0_u32, |hash, byte| {
                (hash.rotate_left(5) ^ u32::from(byte | 0x20)).wrapping_mul(0x9e37_79b9)
            });
            Self::Unknown((hash >> 24) as u8)
        }
    }
}

/// A chained element's chain, and the position of the next element down it,
/// or [`NO_POSITION`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct Link {
    chain: Chain,
    below: DepthSize,
}

/// The innermost chained element on each chain. An element with a known tag
/// id is on that id's chain, and any other element is on one of
/// [`UNKNOWN_CHAINS`] chains by a hash of its name. Each chained element has
/// a [`Link`] to the next one down its chain, so the chains are linked lists
/// through the stack, and chaining or popping an element updates them in
/// constant time.
#[derive(Debug, PartialEq)]
struct TagChains {
    known: [DepthSize; TagId::COUNT],
    unknown: [DepthSize; UNKNOWN_CHAINS],
}

impl Default for TagChains {
    fn default() -> Self {
        Self {
            known: [NO_POSITION; TagId::COUNT],
            unknown: [NO_POSITION; UNKNOWN_CHAINS],
        }
    }
}

impl TagChains {
    #[inline(always)]
    fn innermost_mut(&mut self, chain: Chain) -> &mut DepthSize {
        match chain {
            Chain::Known(id) => &mut self.known[id.index()],
            Chain::Unknown(hash) => &mut self.unknown[usize::from(hash)],
        }
    }

    #[inline(always)]
    fn innermost(&self, chain: Chain) -> Option<usize> {
        let position = match chain {
            Chain::Known(id) => self.known[id.index()],
            Chain::Unknown(hash) => self.unknown[usize::from(hash)],
        };
        (position != NO_POSITION).then_some(usize::from(position))
    }
}

/// First [`TagFlags::INDEXED`] bit; slot `i` of a [`Snapshot`] holds bit
/// `FIRST_INDEXED_BIT + i`.
const FIRST_INDEXED_BIT: u32 = TagFlags::INDEXED.bits().trailing_zeros();
const INDEXED_SLOTS: usize =
    (u32::BITS - TagFlags::INDEXED.bits().leading_zeros() - FIRST_INDEXED_BIT) as usize;

/// The innermost open element with each [`TagFlags::INDEXED`] flag, as seen
/// from one element down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Snapshot {
    /// The indexed flags some element at or below this one carries.
    occupied: u32,
    /// For each occupied flag, the stack position of the innermost element
    /// that carries it.
    innermost: [DepthSize; INDEXED_SLOTS],
}

/// A [`Snapshot`] for each open element with an indexed flag, innermost
/// last, so the last one answers for the whole stack.
///
/// Elements leave the stack top first, so popping an indexed element pops
/// its snapshot and leaves the one that was current before it opened.
#[derive(Debug, Default, PartialEq)]
struct FlagIndex {
    /// The last snapshot's `occupied`, or 0 when there is none.
    occupied: u32,
    snapshots: Vec<Snapshot>,
}

impl FlagIndex {
    #[inline]
    fn push(&mut self, tag: TagFlags, position: usize) {
        let mut snapshot = self.snapshots.last().copied().unwrap_or_default();
        let mut bits = tag.bits() & TagFlags::INDEXED.bits();
        snapshot.occupied |= bits;
        while bits != 0 {
            let slot = (bits.trailing_zeros() - FIRST_INDEXED_BIT) as usize;
            // Positions stay below `MAX_ELEMENT_DEPTH`, so they fit.
            snapshot.innermost[slot] = position as DepthSize;
            bits &= bits - 1;
        }
        self.occupied = snapshot.occupied;
        self.snapshots.push(snapshot);
    }

    #[inline]
    fn pop(&mut self, tag: TagFlags, position: usize) {
        let popped = self.snapshots.pop();
        debug_assert!(popped.is_some_and(|snapshot| {
            let slot = (tag.bits() & TagFlags::INDEXED.bits()).trailing_zeros() - FIRST_INDEXED_BIT;
            usize::from(snapshot.innermost[slot as usize]) == position
        }));
        self.occupied = self
            .snapshots
            .last()
            .map_or(0, |snapshot| snapshot.occupied);
    }

    fn clear(&mut self) {
        self.occupied = 0;
        self.snapshots.clear();
    }

    /// Whether an open element has any flag in `mask`.
    #[inline(always)]
    fn any(&self, mask: TagFlags) -> bool {
        self.occupied & mask.bits() != 0
    }

    /// Position of the innermost open element with any flag in `mask`, which
    /// must be indexed.
    #[inline]
    fn innermost(&self, mask: TagFlags) -> Option<usize> {
        debug_assert_eq!(mask.bits() & !TagFlags::INDEXED.bits(), 0);
        let mut bits = mask.bits() & self.occupied;
        let snapshot = self.snapshots.last().filter(|_| bits != 0)?;
        let mut innermost = 0;
        while bits != 0 {
            let slot = (bits.trailing_zeros() - FIRST_INDEXED_BIT) as usize;
            innermost = innermost.max(usize::from(snapshot.innermost[slot]));
            bits &= bits - 1;
        }
        Some(innermost)
    }
}

#[derive(Debug, PartialEq)]
pub struct OpenElementStack<'html> {
    entries: Vec<OpenElement<'html>>,
    /// `index` and `chains` are kept in step with `entries`: every push and
    /// pop goes through `push_classified`, `pop_entry` or
    /// `close_all_at_eof_into`.
    index: FlagIndex,
    /// Chains of the entries below `links.len()`, and their links. Pushes
    /// leave new entries unchained, and an end tag that needs the chains
    /// chains them first. The links live beside `entries`, not in them, so a
    /// push writes no more than it did before the chains existed.
    chains: TagChains,
    links: Vec<Link>,
}

impl<'html> Default for OpenElementStack<'html> {
    fn default() -> Self {
        const ASSUMED_MAX_DEPTH: usize = 16;
        Self {
            entries: Vec::with_capacity(ASSUMED_MAX_DEPTH),
            index: FlagIndex::default(),
            chains: TagChains::default(),
            links: Vec::new(),
        }
    }
}

impl<'html> OpenElementStack<'html> {
    #[inline]
    pub fn depth(&self) -> DepthSize {
        self.entries.len().try_into().unwrap_or(MAX_ELEMENT_DEPTH)
    }

    #[inline]
    pub const fn would_exceed_max_depth(len: usize) -> bool {
        len >= MAX_ELEMENT_DEPTH as usize
    }

    #[inline(always)]
    pub fn push_classified(
        &mut self,
        name: &'html str,
        id: TagId,
        tag: TagFlags,
        payload: u8,
    ) -> Result<(), DepthExceeded> {
        debug_assert!(u32::from(payload) < 1 << OpenElement::PAYLOAD_BITS);
        if Self::would_exceed_max_depth(self.entries.len()) {
            return Err(DepthExceeded);
        }
        if tag.intersects(TagFlags::INDEXED) {
            self.index.push(tag, self.entries.len());
        }
        self.entries.push(OpenElement {
            name,
            id,
            packed_flags: tag.bits() | (u32::from(payload) << OpenElement::PAYLOAD_SHIFT),
        });
        self.debug_check_index();
        Ok(())
    }

    /// Pop the top entry, keeping the flag index and tag chains in step.
    #[inline(always)]
    fn pop_entry(&mut self) -> Option<OpenElement<'html>> {
        let open = self.entries.pop()?;
        // One test for both, since most elements are neither.
        if open.packed_flags & (TagFlags::INDEXED.bits() | OpenElement::CHAINED) != 0 {
            if open.tag().intersects(TagFlags::INDEXED) {
                self.index.pop(open.tag(), self.entries.len());
            }
            if open.packed_flags & OpenElement::CHAINED != 0 {
                self.unchain();
            }
        }
        Some(open)
    }

    /// Take the element just popped off its chain.
    #[cold]
    #[inline(never)]
    fn unchain(&mut self) {
        let link = self.links.pop().expect("a chained element has a link");
        debug_assert_eq!(self.links.len(), self.entries.len());
        let innermost = self.chains.innermost_mut(link.chain);
        debug_assert_eq!(usize::from(*innermost), self.entries.len());
        *innermost = link.below;
    }

    /// Chain every entry pushed since the last end tag that needed the
    /// chains. Each entry is chained at most once, so over a parse this costs
    /// no more than the pushes did.
    fn chain_open_elements(&mut self) {
        for position in self.links.len()..self.entries.len() {
            let entry = &mut self.entries[position];
            entry.packed_flags |= OpenElement::CHAINED;
            let chain = Chain::of(entry.name, entry.id);
            // Positions stay below `MAX_ELEMENT_DEPTH`, so they fit.
            let below = std::mem::replace(self.chains.innermost_mut(chain), position as DepthSize);
            self.links.push(Link { chain, below });
        }
    }

    /// Pop every entry at `index` and above into `popped`, innermost first.
    #[inline]
    fn pop_down_to_into(&mut self, index: usize, popped: &mut Vec<OpenElement<'html>>) {
        while self.entries.len() > index {
            if let Some(open) = self.pop_entry() {
                popped.push(open);
            }
        }
    }

    /// Check the flag index and tag chains against a rebuild from `entries`.
    ///
    /// The rebuild walks the whole stack, so outside this crate's own tests
    /// it only runs on shallow stacks, which keeps debug builds of deep
    /// documents linear.
    #[inline(always)]
    fn debug_check_index(&self) {
        #[cfg(debug_assertions)]
        if cfg!(test) || self.entries.len() <= 64 {
            let mut rebuilt = FlagIndex::default();
            let mut chains = TagChains::default();
            assert!(self.links.len() <= self.entries.len());
            for (position, entry) in self.entries.iter().enumerate() {
                if entry.tag().intersects(TagFlags::INDEXED) {
                    rebuilt.push(entry.tag(), position);
                }
                let chained = position < self.links.len();
                assert_eq!(
                    entry.packed_flags & OpenElement::CHAINED != 0,
                    chained,
                    "chained flag drifted"
                );
                if chained {
                    let chain = Chain::of(entry.name, entry.id);
                    let below =
                        std::mem::replace(chains.innermost_mut(chain), position as DepthSize);
                    assert_eq!(
                        self.links[position],
                        Link { chain, below },
                        "tag chain drifted"
                    );
                }
            }
            assert_eq!(self.index, rebuilt, "open-element flag index drifted");
            assert_eq!(self.chains, chains, "tag chains drifted");
        }
    }

    #[cfg(test)]
    pub fn push(&mut self, name: &'html str) -> Result<(), DepthExceeded> {
        let id = TagId::of(name);
        self.push_classified(name, id, TagFlags::of(id), 0)
    }

    #[cfg(test)]
    pub fn prepare_for_open(&mut self, name: &str) -> Vec<OpenElement<'html>> {
        let mut popped = Vec::new();
        self.prepare_for_open_into(TagFlags::classify(name), &mut popped);
        popped
    }

    // Called for every start tag. LLVM declines to inline it into the parse
    // loop without a profile; inlining it saves about 1% of cycles.
    #[inline(always)]
    pub fn prepare_for_open_into(&mut self, tag: TagFlags, popped: &mut Vec<OpenElement<'html>>) {
        popped.clear();
        // Usually nothing the tag could close is open.
        if self.index.any(tag.implied_close_targets()) {
            self.close_implied_by_into(tag, popped);
        }
    }

    /// Pop what opening `tag` implicitly closes, once something it could
    /// close is known to be open. Kept out of line, off the common path.
    #[inline(never)]
    fn close_implied_by_into(&mut self, tag: TagFlags, popped: &mut Vec<OpenElement<'html>>) {
        if tag.closes_open_p() {
            self.pop_matching_in_scope_into(TagFlags::P_MASK, ScopeKind::Default, popped);
        }

        if tag.intersects(TagFlags::BUTTON_MASK) {
            self.pop_matching_in_scope_into(TagFlags::BUTTON_MASK, ScopeKind::Button, popped);
        } else if tag.intersects(TagFlags::LI_MASK) {
            self.pop_matching_in_scope_into(TagFlags::LI_MASK, ScopeKind::ListItem, popped);
        } else if tag.intersects(TagFlags::DT_DD_MASK) {
            self.pop_matching_in_scope_into(TagFlags::DT_DD_MASK, ScopeKind::ListItem, popped);
        } else if tag.intersects(TagFlags::OPTION_MASK) {
            self.pop_matching_in_scope_into(TagFlags::OPTION_MASK, ScopeKind::Select, popped);
        } else if tag.intersects(TagFlags::OPTGROUP_MASK) {
            self.pop_matching_in_scope_into(TagFlags::OPTION_MASK, ScopeKind::Select, popped);
            self.pop_matching_in_scope_into(TagFlags::OPTGROUP_MASK, ScopeKind::Select, popped);
        } else if tag.intersects(TagFlags::TR_MASK) {
            self.pop_matching_in_scope_into(TagFlags::TR_MASK, ScopeKind::Table, popped);
        } else if tag.intersects(TagFlags::CELL_MASK) {
            self.pop_matching_in_scope_into(TagFlags::CELL_MASK, ScopeKind::Table, popped);
        }
        self.debug_check_index();
    }

    #[cfg(test)]
    pub fn close_by_end_tag(&mut self, name: &str) -> Vec<OpenElement<'html>> {
        let mut popped = Vec::new();
        self.close_by_end_tag_into(name, TagId::of(name), &mut popped);
        popped
    }

    #[inline]
    pub fn close_by_end_tag_into(
        &mut self,
        name: &str,
        id: TagId,
        popped: &mut Vec<OpenElement<'html>>,
    ) {
        popped.clear();

        if let Some(open) = self.pop_matching_top(name, id) {
            popped.push(open);
            return;
        }

        self.chain_open_elements();
        if let Some(index) = self.find_closed_by(name, id) {
            self.pop_down_to_into(index, popped);
        }
        self.debug_check_index();
    }

    /// Pop the top entry when its name is exactly `name`.
    #[inline(always)]
    pub fn pop_exact_top(&mut self, name: &str) -> Option<OpenElement<'html>> {
        self.entries
            .last()
            .is_some_and(|entry| entry.name == name)
            .then(|| self.pop_entry().expect("matching top element must exist"))
    }

    /// Pop the top entry when it is the element that `name` closes.
    ///
    /// This is the first thing `close_by_end_tag_into` tries: a matching top
    /// wins even if it is also a scope barrier. Callers can take this path
    /// without deriving `close_scope()`.
    #[inline]
    pub fn pop_matching_top(&mut self, name: &str, id: TagId) -> Option<OpenElement<'html>> {
        self.entries
            .last()
            .is_some_and(|entry| entry.closed_by(name, id))
            .then(|| self.pop_entry().expect("matching top element must exist"))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn close_all_at_eof(&mut self) -> Vec<OpenElement<'html>> {
        let mut popped = Vec::new();
        self.close_all_at_eof_into(&mut popped);
        popped
    }

    #[inline]
    pub fn close_all_at_eof_into(&mut self, popped: &mut Vec<OpenElement<'html>>) {
        popped.clear();
        popped.extend(self.entries.drain(..).rev());
        self.index.clear();
        if !self.links.is_empty() {
            self.chains = TagChains::default();
            self.links.clear();
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn pop_matching_in_scope(
        &mut self,
        tags: TagFlags,
        scope: ScopeKind,
    ) -> Vec<OpenElement<'html>> {
        let mut popped = Vec::new();
        self.pop_matching_in_scope_into(tags, scope, &mut popped);
        popped
    }

    fn pop_matching_in_scope_into(
        &mut self,
        tags: TagFlags,
        scope: ScopeKind,
        popped: &mut Vec<OpenElement<'html>>,
    ) {
        if let Some(index) = self.find_first_of(tags, scope) {
            self.pop_down_to_into(index, popped);
        }
    }

    /// Position of the innermost open element with a flag in `tags`, unless
    /// a barrier of `scope` is open above it.
    ///
    /// This is what walking down from the top, stopping at the first element
    /// that matches `tags` or else is a barrier, would find, but it reads the
    /// innermost match and barrier off the flag index instead.
    #[inline]
    fn find_first_of(&self, tags: TagFlags, scope: ScopeKind) -> Option<usize> {
        // The walk's first step, and the usual answer when `tags` is open:
        // a matching top element wins even if it is also a barrier.
        let top = self.entries.len().checked_sub(1)?;
        if self.entries[top].tag().intersects(tags) {
            return Some(top);
        }
        let index = self.index.innermost(tags)?;
        (!self.barrier_above(index, scope)).then_some(index)
    }

    /// Whether a barrier of `scope` is open above `index`.
    #[inline]
    fn barrier_above(&self, index: usize, scope: ScopeKind) -> bool {
        match scope.barriers() {
            Some(barriers) => self
                .index
                .innermost(barriers)
                .is_some_and(|barrier| barrier > index),
            // Select scope's barriers are everything but `option` and
            // `optgroup`, which close each other, so the walk to the first
            // barrier above `index` takes a step or two.
            None => self.entries[index + 1..]
                .iter()
                .rev()
                .any(|entry| entry.tag().is_scope_barrier(scope)),
        }
    }

    /// Position of the innermost open element that an end tag `name`,
    /// resolved to `id`, closes, unless a barrier of the tag's close scope is
    /// open above it.
    ///
    /// This is what walking down from the top, stopping at the first element
    /// the tag closes or else at the first barrier, would find. A known tag
    /// reads the element off its chain, so a stray end tag costs the same at
    /// any depth. Every entry must be chained.
    #[inline]
    fn find_closed_by(&self, name: &str, id: TagId) -> Option<usize> {
        let chain = Chain::of(name, id);
        let mut index = self.chains.innermost(chain)?;
        let scope = TagFlags::of(id).close_scope();
        if let Chain::Known(_) = chain {
            return (!self.barrier_above(index, scope)).then_some(index);
        }
        // Other unknown names can share the chain. Skipping them stops at the
        // innermost barrier, where the walk would stop, so only open elements
        // whose names share the chain and are in scope cost a step.
        debug_assert_eq!(scope, ScopeKind::Default);
        let floor = self.index.innermost(DEFAULT_BARRIERS);
        loop {
            if floor.is_some_and(|barrier| barrier > index) {
                return None;
            }
            if self.entries[index].closed_by(name, id) {
                return Some(index);
            }
            let below = self.links[index].below;
            if below == NO_POSITION {
                return None;
            }
            index = usize::from(below);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chain, MAX_ELEMENT_DEPTH, OpenElement, OpenElementStack};
    use crate::TagId;
    use crate::tag::{ScopeKind, TagFlags};

    #[test]
    fn would_exceed_max_depth_at_boundary() {
        assert!(!OpenElementStack::would_exceed_max_depth(
            MAX_ELEMENT_DEPTH as usize - 1
        ));
        assert!(OpenElementStack::would_exceed_max_depth(
            MAX_ELEMENT_DEPTH as usize
        ));
    }

    #[test]
    fn tag_flags_fit_below_the_chained_bit() {
        for index in 0..TagId::COUNT {
            let tag = TagFlags::of(TagId::from_index(index));
            assert!(tag.bits() < OpenElement::CHAINED, "{index}");
        }
    }

    #[test]
    fn open_element_keeps_hot_stack_entries_compact() {
        assert!(std::mem::size_of::<OpenElement<'_>>() <= 24);
    }

    #[test]
    fn test_misnested_close_bubbles_to_match() {
        let mut stack = OpenElementStack::default();
        stack.push("div").unwrap();
        stack.push("span").unwrap();

        let popped = stack.close_by_end_tag("div");
        assert_eq!(popped.len(), 2);
        assert_eq!(popped[0].name, "span");
        assert_eq!(popped[1].name, "div");
    }

    #[test]
    fn test_well_formed_mixed_case_close_pops_top() {
        let mut stack = OpenElementStack::default();
        stack.push("div").unwrap();
        stack.push("SpAn").unwrap();

        let popped = stack.close_by_end_tag("span");
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].name, "SpAn");
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_stray_close_is_ignored() {
        let mut stack = OpenElementStack::default();
        stack.push("div").unwrap();

        let popped = stack.close_by_end_tag("span");
        assert!(popped.is_empty());
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_opening_li_closes_previous_li() {
        let mut stack = OpenElementStack::default();
        stack.push("ul").unwrap();
        stack.push("li").unwrap();

        let popped = stack.prepare_for_open("li");
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].name, "li");
    }

    #[test]
    fn test_opening_option_closes_previous_option() {
        let mut stack = OpenElementStack::default();
        stack.push("select").unwrap();
        stack.push("option").unwrap();

        let popped = stack.prepare_for_open("option");
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].name, "option");
    }

    #[test]
    fn test_opening_optgroup_closes_option_then_optgroup() {
        let mut stack = OpenElementStack::default();
        stack.push("select").unwrap();
        stack.push("optgroup").unwrap();
        stack.push("option").unwrap();

        let popped = stack.prepare_for_open("optgroup");
        assert_eq!(popped.len(), 2);
        assert_eq!(popped[0].name, "option");
        assert_eq!(popped[1].name, "optgroup");
    }

    #[test]
    fn test_opening_td_closes_previous_cell() {
        let mut stack = OpenElementStack::default();
        stack.push("table").unwrap();
        stack.push("tr").unwrap();
        stack.push("td").unwrap();

        let popped = stack.prepare_for_open("td");
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].name, "td");
    }

    #[test]
    fn test_opening_button_closes_previous_button() {
        let mut stack = OpenElementStack::default();
        stack.push("div").unwrap();
        stack.push("button").unwrap();

        let popped = stack.prepare_for_open("button");
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].name, "button");
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_select_scope_ignores_non_select_end_tags() {
        let mut stack = OpenElementStack::default();
        stack.push("select").unwrap();
        stack.push("option").unwrap();

        let popped = stack.close_by_end_tag("div");
        assert!(popped.is_empty());
        assert_eq!(stack.depth(), 2);
    }

    fn names(stack: &OpenElementStack<'_>) -> Vec<String> {
        stack.entries.iter().map(|e| e.name.to_owned()).collect()
    }

    fn open(stack: &mut OpenElementStack<'static>, name: &'static str) -> Vec<&'static str> {
        let popped = stack
            .prepare_for_open(name)
            .iter()
            .map(|e| e.name)
            .collect();
        if !TagFlags::classify(name).is_void() {
            stack.push(name).unwrap();
        }
        popped
    }

    #[test]
    fn test_deep_chain_then_p_and_li_close_correctly() {
        let mut stack = OpenElementStack::default();
        open(&mut stack, "ul");
        open(&mut stack, "li");
        for _ in 0..2_000 {
            assert!(open(&mut stack, "section").is_empty());
        }
        assert_eq!(open(&mut stack, "p"), Vec::<&str>::new());
        assert_eq!(open(&mut stack, "div"), vec!["p"]);
        assert_eq!(stack.close_by_end_tag("div").len(), 1);
        for _ in 0..2_000 {
            assert_eq!(stack.close_by_end_tag("section").len(), 1);
        }
        open(&mut stack, "span");
        assert_eq!(open(&mut stack, "li"), vec!["span", "li"]);
        assert_eq!(names(&stack), ["ul", "li"]);
    }

    #[test]
    fn test_barrier_deep_below_top_blocks_implied_close() {
        let mut stack = OpenElementStack::default();
        open(&mut stack, "p");
        open(&mut stack, "td");
        for _ in 0..2_000 {
            assert!(open(&mut stack, "div").is_empty());
        }
        let popped = stack.close_by_end_tag("td");
        assert_eq!(popped.len(), 2_001);
        assert_eq!(open(&mut stack, "div"), vec!["p"]);
    }

    #[test]
    fn test_eof_drain_resets_index() {
        let mut stack = OpenElementStack::default();
        open(&mut stack, "table");
        open(&mut stack, "tr");
        open(&mut stack, "td");
        open(&mut stack, "p");
        assert_eq!(stack.close_all_at_eof().len(), 4);
        open(&mut stack, "div");
        assert_eq!(open(&mut stack, "td"), Vec::<&str>::new());
        assert_eq!(open(&mut stack, "p"), Vec::<&str>::new());
        assert_eq!(open(&mut stack, "tr"), Vec::<&str>::new());
        assert_eq!(names(&stack), ["div", "td", "p", "tr"]);
    }

    #[test]
    fn test_stray_end_tags_leave_deep_stack_alone() {
        let mut stack = OpenElementStack::default();
        for _ in 0..2_000 {
            open(&mut stack, "div");
        }
        open(&mut stack, "x-a");
        for name in ["span", "p", "li", "td", "option", "x-b", "X-B", "x-636"] {
            assert!(stack.close_by_end_tag(name).is_empty(), "</{name}>");
        }
        assert_eq!(stack.close_by_end_tag("div").len(), 2);
        assert_eq!(stack.depth(), 1_999);
    }

    #[test]
    fn test_end_tag_below_barrier_is_ignored() {
        let mut stack = OpenElementStack::default();
        open(&mut stack, "span");
        open(&mut stack, "x-a");
        open(&mut stack, "table");
        for _ in 0..2_000 {
            open(&mut stack, "div");
        }
        assert!(stack.close_by_end_tag("span").is_empty());
        assert!(stack.close_by_end_tag("x-a").is_empty());
        let popped = stack.close_by_end_tag("table");
        assert_eq!(popped.len(), 2_001);
        assert_eq!(stack.close_by_end_tag("X-A").len(), 1);
        assert_eq!(names(&stack), ["span"]);
    }

    #[test]
    fn test_unknown_names_sharing_a_chain_close_by_name() {
        // `x-636` hashes to the same chain as `x-a`.
        assert_eq!(
            Chain::of("x-636", TagId::UNKNOWN),
            Chain::of("x-a", TagId::UNKNOWN)
        );
        let mut stack = OpenElementStack::default();
        open(&mut stack, "X-A");
        open(&mut stack, "x-636");
        open(&mut stack, "div");
        open(&mut stack, "x-636");
        assert_eq!(
            stack
                .close_by_end_tag("x-a")
                .iter()
                .map(|e| e.name)
                .collect::<Vec<_>>(),
            ["x-636", "div", "x-636", "X-A"]
        );
        open(&mut stack, "x-636");
        open(&mut stack, "object");
        open(&mut stack, "x-a");
        open(&mut stack, "x-636");
        assert_eq!(stack.close_by_end_tag("x-a").len(), 2);
        assert!(stack.close_by_end_tag("x-a").is_empty());
        assert!(stack.close_by_end_tag("X-636").is_empty());
        assert_eq!(names(&stack), ["x-636", "object"]);
    }

    /// The stack before it indexed flags: every implied close and end tag
    /// walks down from the top.
    #[derive(Default)]
    struct WalkingStack(Vec<(&'static str, TagFlags)>);

    impl WalkingStack {
        fn pop_in_scope(&mut self, tags: TagFlags, scope: ScopeKind, popped: &mut Vec<&str>) {
            for index in (0..self.0.len()).rev() {
                let tag = self.0[index].1;
                if tag.intersects(tags) {
                    popped.extend(self.0.drain(index..).rev().map(|(name, _)| name));
                    return;
                }
                if tag.is_scope_barrier(scope) {
                    return;
                }
            }
        }

        fn open(&mut self, name: &'static str) -> Vec<&'static str> {
            let tag = TagFlags::classify(name);
            let mut popped = Vec::new();
            if tag.closes_open_p() {
                self.pop_in_scope(TagFlags::P_MASK, ScopeKind::Default, &mut popped);
            }
            if tag.intersects(TagFlags::BUTTON_MASK) {
                self.pop_in_scope(TagFlags::BUTTON_MASK, ScopeKind::Button, &mut popped);
            } else if tag.intersects(TagFlags::LI_MASK) {
                self.pop_in_scope(TagFlags::LI_MASK, ScopeKind::ListItem, &mut popped);
            } else if tag.intersects(TagFlags::DT_DD_MASK) {
                self.pop_in_scope(TagFlags::DT_DD_MASK, ScopeKind::ListItem, &mut popped);
            } else if tag.intersects(TagFlags::OPTION_MASK) {
                self.pop_in_scope(TagFlags::OPTION_MASK, ScopeKind::Select, &mut popped);
            } else if tag.intersects(TagFlags::OPTGROUP_MASK) {
                self.pop_in_scope(TagFlags::OPTION_MASK, ScopeKind::Select, &mut popped);
                self.pop_in_scope(TagFlags::OPTGROUP_MASK, ScopeKind::Select, &mut popped);
            } else if tag.intersects(TagFlags::TR_MASK) {
                self.pop_in_scope(TagFlags::TR_MASK, ScopeKind::Table, &mut popped);
            } else if tag.intersects(TagFlags::CELL_MASK) {
                self.pop_in_scope(TagFlags::CELL_MASK, ScopeKind::Table, &mut popped);
            }
            if !tag.is_void() {
                self.0.push((name, tag));
            }
            popped
        }

        fn close(&mut self, name: &str) -> Vec<&'static str> {
            let id = TagId::of(name);
            let scope = TagFlags::of(id).close_scope();
            for index in (0..self.0.len()).rev() {
                let (open, tag) = self.0[index];
                let closes = if id.is_known() || TagId::of(open).is_known() {
                    TagId::of(open) == id
                } else {
                    open.eq_ignore_ascii_case(name)
                };
                if closes {
                    return self.0.drain(index..).rev().map(|(name, _)| name).collect();
                }
                if tag.is_scope_barrier(scope) {
                    break;
                }
            }
            Vec::new()
        }
    }

    /// Random opens, closes and EOF drains over every tag the implied
    /// closes and end tags care about pop exactly what the walking stack
    /// pops. Unknown names include mixed case and names that share a chain.
    #[test]
    fn test_indexed_closes_match_walking_stack() {
        const TAGS: &[&str] = &[
            "p",
            "div",
            "section",
            "span",
            "b",
            "button",
            "li",
            "ul",
            "ol",
            "dt",
            "dd",
            "dl",
            "option",
            "optgroup",
            "select",
            "table",
            "tbody",
            "tr",
            "td",
            "th",
            "caption",
            "object",
            "marquee",
            "applet",
            "template",
            "html",
            "hr",
            "br",
            "img",
            "script",
            "style",
            "title",
            "textarea",
            "DIV",
            "Span",
            "TD",
            "x-a",
            "X-A",
            "x-636",
            "x-784",
            "x-b",
            "my-widget",
            "My-Widget",
            "averyveryverylongtagname",
        ];
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for _ in 0..200 {
            let mut stack = OpenElementStack::default();
            let mut walking = WalkingStack::default();
            for _ in 0..400 {
                let roll = next();
                let name = TAGS[(roll >> 8) as usize % TAGS.len()];
                // Open three times as often as close, so stacks grow deep.
                if roll % 4 != 0 {
                    assert_eq!(open(&mut stack, name), walking.open(name), "<{name}>");
                } else {
                    let popped: Vec<_> = stack
                        .close_by_end_tag(name)
                        .iter()
                        .map(|e| e.name)
                        .collect();
                    assert_eq!(popped, walking.close(name), "</{name}>");
                }
                assert_eq!(
                    names(&stack),
                    walking.0.iter().map(|e| e.0).collect::<Vec<_>>()
                );
            }
            assert_eq!(stack.close_all_at_eof().len(), walking.0.len());
        }
    }
}
