//! Flat, immutable compilation of a query set.
//!
//! A [`Program`] numbers every compound selector ("step") of every query,
//! section, and selector-list alternative, and stores what the matcher needs
//! about them as columns: one bit per step in a handful of masks, plus a few
//! per-step and per-section side tables.
//!
//! Steps of one alternative are numbered consecutively, so the step a match
//! enables next is always `s + 1`. A matcher can therefore advance every
//! partial match at once with a shift and a few `AND`s against these masks,
//! instead of walking cursors through a transition tree.
//!
//! ```text
//! queries:  div > p  |  a.x   ⟶ then ⟶  span
//! steps:      0   1      2               3
//! ChildIn:        ●
//! RootEntryDescendant: ●       ●
//! Save:           ●      ●               ●
//! Scoped:                                ●
//! ```

pub mod bits;
mod compile;

use crate::query::compiler::{
    QuerySection, SelectionKind, TextRequirements, Transition, ascii_case_insensitive_hash,
};
use crate::query::selector::{
    ElementPredicate, LocalLogicalPredicate, LocalSelectorList, StructuralPredicate,
};
use crate::tag::TagId;

/// Marks a known tag that no type selector names.
const NO_NAME: u32 = u32::MAX;

/// A step-wide mask stored once per program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum StepMask {
    /// Non-entry steps reached from the previous step through ` `.
    DescendantIn,
    /// Non-entry steps reached from the previous step through `>`.
    ChildIn,
    /// Non-entry steps reached from the previous step through `+`.
    AdjacentIn,
    /// Non-entry steps reached from the previous step through `~`.
    SiblingIn,
    /// First steps of root sections that may match at any depth.
    RootEntryDescendant,
    /// First steps of root sections that may only match top-level elements.
    RootEntryChild,
    /// Steps of nested sections. Their matches belong to specific scope
    /// elements, so the matcher tracks which scopes each partial match is
    /// anchored in.
    Scoped,
    /// Last steps of an alternative: a match here saves the element.
    Save,
    /// Save steps whose section keeps the element's attributes.
    SaveAttributes,
    /// Steps whose predicate needs more than the tag-name check.
    Filter,
    /// Steps without a type selector (`*`, `.x`, `[href]`, ...).
    Universal,
    /// Steps read by a child or later sibling: followed by `>`, `+`, or `~`.
    ChildRead,
    /// Steps read by descendants: followed by ` `.
    DescendantRead,
    /// Steps only descendants read that neither save nor are scoped:
    /// matching one again under an ancestor that matched it adds nothing.
    RedundantIfInherited,
}

const STEP_MASK_COUNT: usize = StepMask::RedundantIfInherited as usize + 1;

/// A mask stored once per section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SectionMask {
    /// Every step of the section.
    Steps,
    /// The section's save steps.
    Save,
    /// First steps entered from a scope element through ` `.
    EntryDescendant,
    /// First steps entered from a scope element through `>`.
    EntryChild,
    /// [`SectionMask::EntryDescendant`] of every child section: what an open
    /// scope of this section enables in its subtree.
    ChildEntryDescendant,
    /// [`SectionMask::EntryChild`] of every child section: what an open scope
    /// of this section enables for its children.
    ChildEntryChild,
}

const SECTION_MASK_COUNT: usize = SectionMask::ChildEntryChild as usize + 1;

/// How a nested-section step learns which scopes its match is anchored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneSource {
    /// First step, entered through ` ` from any open scope.
    EntryDescendant,
    /// First step, entered through `>` from a scope that is the parent.
    EntryChild,
    /// From the previous step's lane in the nearest ancestor.
    Descendant,
    /// From the previous step's lane in the parent.
    Child,
    /// From the previous step's lane in the previous sibling.
    Adjacent,
    /// From the previous step's lane in any earlier sibling.
    Sibling,
    /// Root-section steps, and steps that never match.
    Never,
}

/// What a matcher does with a step: everything it needs, in one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepPlan {
    pub section: SectionIndex,
    /// Lane index among scoped steps; `u32::MAX` for root-section steps.
    pub lane: u32,
    pub lane_source: LaneSource,
    /// Position of the section in its parent's child list.
    pub child: u32,
    pub root: bool,
    pub first: bool,
    pub has_children: bool,
}

/// Index of a section across the whole program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionIndex(pub u32);

impl SectionIndex {
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Attributes a step inspects, as bits over [`Program::attribute_names`].
///
/// `id` and `class` have dedicated flags because the parser stores them in
/// dedicated element fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttributeMask {
    pub flags: u8,
    /// Bit `i` stands for `attribute_names()[i]`.
    pub keys: u64,
}

impl AttributeMask {
    pub const ID: u8 = 1;
    pub const CLASS: u8 = 1 << 1;
    /// Every attribute: the step saves them, or names overflowed `keys`.
    pub const ALL: u8 = 1 << 2;

    #[inline(always)]
    pub fn union(self, other: Self) -> Self {
        Self {
            flags: self.flags | other.flags,
            keys: self.keys | other.keys,
        }
    }

    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.flags == 0 && self.keys == 0
    }
}

/// Query features that decide which matcher and parser state is needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProgramFeatures {
    /// Some step is reached through `+` or `~`.
    pub has_siblings: bool,
    /// Some predicate needs structural context (ordinals, `:root`, ...).
    pub has_structural: bool,
    pub needs_child_ordinals: bool,
    pub needs_type_ordinals: bool,
    pub needs_filtered_ordinals: bool,
    /// Some section is a selector list with more than one alternative.
    pub has_selector_lists: bool,
    /// Some section is nested under another one.
    pub has_scoped_sections: bool,
    /// Every root section is `First`, so parsing can stop once all of them
    /// have finished.
    pub all_roots_first: bool,
    /// Text representations requested by any section.
    pub text: TextRequirements,
    /// Some section keeps matched elements' attributes.
    pub stores_attributes: bool,
    /// Attributes must be tokenized, for matching or for storage.
    pub parses_attributes: bool,
}

/// Per-step scalars, read together when a step matches.
#[derive(Debug, Clone, Copy)]
struct StepInfo<'q> {
    transition: &'q Transition<'q>,
    plan: StepPlan,
    interest: AttributeMask,
}

/// Per-section scalars.
#[derive(Debug, Clone, Copy)]
struct SectionInfo<'q> {
    spec: &'q QuerySection<'q>,
    parent: Option<SectionIndex>,
    /// Range of this section's children in `Program::links`.
    children: (u32, u32),
}

/// Every query of one parse, compiled into flat step and section tables.
///
/// Storage is grouped by access pattern into a handful of allocations: one
/// arena of `u64` masks (step masks, then section masks, then name masks),
/// one row per step, one row per section, and shared index and string lists.
#[derive(Debug, Clone)]
pub struct Program<'q> {
    words: usize,
    steps: Box<[StepInfo<'q>]>,
    sections: Box<[SectionInfo<'q>]>,
    /// Every section's children, followed by the root sections.
    links: Box<[SectionIndex]>,
    roots_start: usize,

    masks: Box<[u64]>,
    section_masks_start: usize,
    name_masks_start: usize,

    /// Type-selector names, followed by attribute names.
    strings: Box<[&'q str]>,
    attributes_start: usize,
    /// Packed keys of short type names, sorted, then hashes of long ones; each
    /// with its index into `strings`.
    name_keys: Box<[(u64, u32)]>,
    hashes_start: usize,
    /// Bit `n` is set when some type name is `n` bytes long (`n < 63`; longer
    /// names share bit 63). Most tags have a length no query name has.
    name_lengths: u64,
    /// Type-name entry of each known tag, so known tags skip name lookup.
    tag_names: [u32; TagId::COUNT],

    /// Attributes `:nth-child(An+B of S)` filters inspect on every element.
    filter_interest: AttributeMask,
    /// Steps of nested sections, each with a lane.
    lane_count: usize,
    /// Most child sections of any section, in words of a claims bitset.
    claim_words: usize,
    features: ProgramFeatures,
}

impl<'q> Program<'q> {
    /// Words per step mask.
    #[inline(always)]
    pub fn words(&self) -> usize {
        self.words
    }

    #[inline(always)]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    #[inline(always)]
    pub fn features(&self) -> &ProgramFeatures {
        &self.features
    }

    /// Word `word` of a step mask, for callers that know the width
    /// statically: `words` must equal [`Program::words`].
    #[inline(always)]
    pub fn mask_word(&self, mask: StepMask, word: usize, words: usize) -> u64 {
        debug_assert_eq!(words, self.words);
        self.masks[mask as usize * words + word]
    }

    /// Word `word` of a section mask; see [`Program::mask_word`].
    #[inline(always)]
    pub fn section_mask_word(
        &self,
        section: SectionIndex,
        mask: SectionMask,
        word: usize,
        words: usize,
    ) -> u64 {
        debug_assert_eq!(words, self.words);
        self.masks[STEP_MASK_COUNT * words
            + (section.index() * SECTION_MASK_COUNT + mask as usize) * words
            + word]
    }

    #[inline(always)]
    pub fn mask(&self, mask: StepMask) -> &[u64] {
        let start = mask as usize * self.words;
        &self.masks[start..start + self.words]
    }

    /// Steps whose type selector is `name`, compared ASCII case-insensitively.
    /// Universal steps are not included; see [`StepMask::Universal`].
    #[inline(always)]
    pub fn name_mask(&self, name: &str) -> Option<&[u64]> {
        let entry = self.find_name(name)?;
        let start = self.name_masks_start + entry * self.words;
        Some(&self.masks[start..start + self.words])
    }

    /// [`Program::name_mask`] for a tag already resolved to `tag`: known tags
    /// are a table lookup, other names fall back to comparing `name`.
    #[inline(always)]
    pub fn tag_mask(&self, tag: TagId, name: &str) -> Option<&[u64]> {
        if !tag.is_known() {
            return self.name_mask(name);
        }
        match self.tag_names[tag.index()] {
            NO_NAME => None,
            entry => {
                let start = self.name_masks_start + entry as usize * self.words;
                Some(&self.masks[start..start + self.words])
            }
        }
    }

    #[inline(always)]
    fn find_name(&self, name: &str) -> Option<usize> {
        if self.name_lengths & (1 << name.len().min(63)) == 0 {
            return None;
        }
        if let Some(raw) = packed_name(name.as_bytes()) {
            // Keys are stored lowercase; most documents use lowercase tags.
            if let Some(entry) = self.find_packed(raw) {
                return Some(entry);
            }
            let key = lowercase_packed(raw);
            return if key == raw {
                None
            } else {
                self.find_packed(key)
            };
        }
        let hashed = &self.name_keys[self.hashes_start..];
        let hash = ascii_case_insensitive_hash(name);
        let mut index = hashed.partition_point(|(candidate, _)| *candidate < hash);
        while let Some(&(candidate, entry)) = hashed.get(index) {
            if candidate != hash {
                break;
            }
            if self.strings[entry as usize].eq_ignore_ascii_case(name) {
                return Some(entry as usize);
            }
            index += 1;
        }
        None
    }

    #[inline(always)]
    fn find_packed(&self, key: u64) -> Option<usize> {
        let packed = &self.name_keys[..self.hashes_start];
        let index = if packed.len() <= 8 {
            packed.iter().position(|(candidate, _)| *candidate == key)?
        } else {
            packed
                .binary_search_by_key(&key, |(candidate, _)| *candidate)
                .ok()?
        };
        Some(packed[index].1 as usize)
    }

    /// Attribute names indexed by [`AttributeMask::keys`].
    #[inline(always)]
    pub fn attribute_names(&self) -> &[&'q str] {
        &self.strings[self.attributes_start..]
    }

    /// Attributes a step inspects, or [`AttributeMask::ALL`] when its match
    /// saves them.
    #[inline(always)]
    pub fn step_interest(&self, step: usize) -> AttributeMask {
        self.steps[step].interest
    }

    /// Attributes structural filters inspect on every element.
    #[inline(always)]
    pub fn filter_interest(&self) -> AttributeMask {
        self.filter_interest
    }

    #[inline(always)]
    pub fn transition(&self, step: usize) -> &'q Transition<'q> {
        self.steps[step].transition
    }

    #[inline(always)]
    pub fn predicate(&self, step: usize) -> &'q ElementPredicate<'q> {
        self.steps[step].transition.predicate()
    }

    #[inline(always)]
    pub fn step_section(&self, step: usize) -> SectionIndex {
        self.steps[step].plan.section
    }

    /// Everything a matcher does with `step`.
    #[inline(always)]
    pub fn plan(&self, step: usize) -> StepPlan {
        self.steps[step].plan
    }

    /// Number of nested-section steps, each with a lane.
    #[inline(always)]
    pub fn lane_count(&self) -> usize {
        self.lane_count
    }

    /// Words of a bitset over the child sections of any section.
    #[inline(always)]
    pub fn claim_words(&self) -> usize {
        self.claim_words
    }

    #[inline(always)]
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    #[inline(always)]
    pub fn section(&self, section: SectionIndex) -> &'q QuerySection<'q> {
        self.sections[section.index()].spec
    }

    #[inline(always)]
    pub fn section_parent(&self, section: SectionIndex) -> Option<SectionIndex> {
        self.sections[section.index()].parent
    }

    #[inline(always)]
    pub fn section_kind(&self, section: SectionIndex) -> SelectionKind {
        self.sections[section.index()].spec.kind
    }

    #[inline(always)]
    pub fn section_mask(&self, section: SectionIndex, mask: SectionMask) -> &[u64] {
        let start = self.section_masks_start
            + (section.index() * SECTION_MASK_COUNT + mask as usize) * self.words;
        &self.masks[start..start + self.words]
    }

    /// Sections nested directly under `section`, in declaration order.
    #[inline(always)]
    pub fn children(&self, section: SectionIndex) -> &[SectionIndex] {
        let (start, end) = self.sections[section.index()].children;
        &self.links[start as usize..end as usize]
    }

    #[inline(always)]
    pub fn has_children(&self, section: SectionIndex) -> bool {
        let (start, end) = self.sections[section.index()].children;
        start != end
    }

    #[inline(always)]
    pub fn root_sections(&self) -> &[SectionIndex] {
        &self.links[self.roots_start..]
    }

    /// Distinct `:nth-child(An+B of S)` filter lists, by identity.
    pub fn structural_filters(&self) -> Vec<&'q LocalSelectorList<'q>> {
        let transitions: Vec<_> = self.steps.iter().map(|step| step.transition).collect();
        structural_filters(&transitions)
    }

    /// Names whose `:nth-of-type` ordinals must be counted, or `None` when a
    /// universal `*:nth-of-type` needs every name.
    pub fn type_ordinal_names(&self) -> Option<Vec<&'q str>> {
        let mut names: Vec<&'q str> = Vec::new();
        let mut needs_all = false;
        for step in self.steps.iter() {
            visit_structural_with_owner(step.transition.predicate(), &mut |owner, structural| {
                if !matches!(
                    structural,
                    StructuralPredicate::FirstOfType | StructuralPredicate::NthOfType(_)
                ) {
                    return;
                }
                match owner.name {
                    Some(name) => {
                        if !names
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(name))
                        {
                            names.push(name);
                        }
                    }
                    None => needs_all = true,
                }
            });
        }
        (!needs_all).then_some(names)
    }
}

/// Names of up to this many bytes are compared as one packed `u64` key.
const PACKED_NAME_LEN: usize = 7;

/// Bytes of a short name, with its length in the top byte.
#[inline(always)]
fn packed_name(name: &[u8]) -> Option<u64> {
    if name.len() > PACKED_NAME_LEN {
        return None;
    }
    let mut key = (name.len() as u64) << 56;
    for (index, byte) in name.iter().enumerate() {
        key |= (*byte as u64) << (8 * index);
    }
    Some(key)
}

/// ASCII-lowercase every byte of a packed name (SWAR).
#[inline(always)]
const fn lowercase_packed(key: u64) -> u64 {
    const LOW_BITS: u64 = 0x7f7f_7f7f_7f7f_7f7f;
    const HIGH_BITS: u64 = 0x8080_8080_8080_8080;
    let bytes = key & 0x00ff_ffff_ffff_ffff;
    let heptets = bytes & LOW_BITS;
    // High bit set where a byte is above 'Z', and where it is at least 'A'.
    let above_z = heptets + 0x2525_2525_2525_2525;
    let at_least_a = heptets + 0x3f3f_3f3f_3f3f_3f3f;
    let upper = !bytes & (at_least_a ^ above_z) & HIGH_BITS;
    key | (upper >> 2)
}

fn structural_filters<'q>(transitions: &[&'q Transition<'q>]) -> Vec<&'q LocalSelectorList<'q>> {
    let mut filters: Vec<&'q LocalSelectorList<'q>> = Vec::new();
    for transition in transitions {
        visit_structural(transition.predicate(), &mut |structural| {
            if let StructuralPredicate::NthChildOf(_, filter) = structural
                && !filters
                    .iter()
                    .any(|existing| std::ptr::eq(*existing, filter))
            {
                filters.push(filter);
            }
        });
    }
    filters
}

fn visit_structural<'p, 'q>(
    predicate: &'p ElementPredicate<'q>,
    visitor: &mut impl FnMut(&'p StructuralPredicate<'q>),
) {
    visit_structural_with_owner(predicate, &mut |_, structural| visitor(structural));
}

fn visit_structural_with_owner<'p, 'q>(
    predicate: &'p ElementPredicate<'q>,
    visitor: &mut impl FnMut(&'p ElementPredicate<'q>, &'p StructuralPredicate<'q>),
) {
    for structural in predicate.structural.as_slice() {
        visitor(predicate, structural);
    }
    for logical in predicate.logical.as_slice() {
        let (LocalLogicalPredicate::Not(list) | LocalLogicalPredicate::Any(list)) = logical;
        for nested in list.as_slice() {
            visit_structural_with_owner(nested, visitor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bits::iter_ones;
    use super::*;
    use crate::{Query, Save};

    fn ones(mask: &[u64]) -> Vec<usize> {
        iter_ones(mask).collect()
    }

    #[test]
    fn numbers_steps_across_queries_and_sections() {
        let queries = [
            Query::all("div > p", Save::all()).unwrap().build(),
            Query::first("a.x", Save::none())
                .unwrap()
                .all("span", Save::name_only())
                .unwrap()
                .build(),
        ];
        let program = Program::compile(&queries);

        assert_eq!(program.step_count(), 4);
        assert_eq!(program.section_count(), 3);
        assert_eq!(ones(program.mask(StepMask::RootEntryDescendant)), [0, 2]);
        assert_eq!(ones(program.mask(StepMask::ChildIn)), [1]);
        assert_eq!(ones(program.mask(StepMask::Save)), [1, 2, 3]);
        assert_eq!(ones(program.mask(StepMask::SaveAttributes)), [1, 2]);
        assert_eq!(ones(program.mask(StepMask::Scoped)), [3]);
        assert_eq!(ones(program.mask(StepMask::Filter)), [2]);
        assert_eq!(program.root_sections(), [SectionIndex(0), SectionIndex(1)]);
        assert_eq!(program.children(SectionIndex(1)), [SectionIndex(2)]);
        assert_eq!(
            program.section_parent(SectionIndex(2)),
            Some(SectionIndex(1))
        );
        assert_eq!(
            ones(program.section_mask(SectionIndex(2), SectionMask::EntryDescendant)),
            [3]
        );
        assert!(!program.features().all_roots_first);
        assert!(program.features().has_scoped_sections);
    }

    #[test]
    fn selector_list_alternatives_do_not_chain_into_each_other() {
        let queries = [Query::all("article > p, section > div", Save::none())
            .unwrap()
            .build()];
        let program = Program::compile(&queries);

        assert_eq!(ones(program.mask(StepMask::RootEntryDescendant)), [0, 2]);
        assert_eq!(ones(program.mask(StepMask::ChildIn)), [1, 3]);
        assert_eq!(ones(program.mask(StepMask::Save)), [1, 3]);
        assert!(program.features().has_selector_lists);

        // The end of the first alternative must not enable the start of the second.
        let mut enabled = [0_u64];
        bits::or_shifted_and(&mut enabled, &[0b0010], program.mask(StepMask::ChildIn));
        assert_eq!(enabled, [0]);
    }

    #[test]
    fn scoped_child_entries_are_split_by_combinator() {
        let queries = [Query::all("main", Save::none())
            .unwrap()
            .then(|main| {
                Ok([
                    main.all("> a", Save::none())?,
                    main.all("ul li", Save::none())?,
                ])
            })
            .unwrap()
            .build()];
        let program = Program::compile(&queries);

        assert_eq!(
            program.children(SectionIndex(0)),
            [SectionIndex(1), SectionIndex(2)]
        );
        assert_eq!(
            ones(program.section_mask(SectionIndex(1), SectionMask::EntryChild)),
            [1]
        );
        assert_eq!(
            ones(program.section_mask(SectionIndex(2), SectionMask::EntryDescendant)),
            [2]
        );
        assert_eq!(ones(program.mask(StepMask::DescendantIn)), [3]);
        assert_eq!(ones(program.mask(StepMask::Scoped)), [1, 2, 3]);
    }

    #[test]
    fn name_lookup_is_case_insensitive_and_merges_steps() {
        let queries = [Query::all("DIV p, div", Save::none()).unwrap().build()];
        let program = Program::compile(&queries);

        assert_eq!(ones(program.name_mask("div").unwrap()), [0, 2]);
        assert_eq!(ones(program.name_mask("Div").unwrap()), [0, 2]);
        assert_eq!(ones(program.name_mask("P").unwrap()), [1]);
        assert!(program.name_mask("span").is_none());
    }

    #[test]
    fn name_lookup_handles_long_names_and_large_tables() {
        let selector = "a, b, i, p, ul, li, em, td, tr, th, section, blockquote, custom-element";
        let queries = [Query::all(selector, Save::none()).unwrap().build()];
        let program = Program::compile(&queries);

        for (step, name) in selector.split(", ").enumerate() {
            assert_eq!(ones(program.name_mask(name).unwrap()), [step], "{name}");
            let upper = name.to_ascii_uppercase();
            assert_eq!(ones(program.name_mask(&upper).unwrap()), [step], "{upper}");
        }
        assert!(program.name_mask("div").is_none());
        assert!(program.name_mask("blockquotes").is_none());
        assert!(program.name_mask("sectio").is_none());
    }

    #[test]
    fn packed_lowercase_changes_only_ascii_capitals() {
        for byte in 0..=255_u8 {
            let key = packed_name(&[byte, b'A', b'z']).unwrap();
            let expected = packed_name(&[byte.to_ascii_lowercase(), b'a', b'z']).unwrap();
            assert_eq!(lowercase_packed(key), expected, "byte {byte:#x}");
        }
    }

    #[test]
    fn attribute_interest_is_a_mask_over_shared_names() {
        let queries = [
            Query::all(
                "a[href].x > b#y[data-k], li:nth-child(odd of [title])",
                Save::name_only(),
            )
            .unwrap()
            .build(),
            Query::all("[HREF]", Save::all()).unwrap().build(),
        ];
        let program = Program::compile(&queries);

        assert_eq!(program.attribute_names(), ["href", "data-k", "title"]);
        assert_eq!(
            program.step_interest(0),
            AttributeMask {
                flags: AttributeMask::CLASS,
                keys: 0b001
            }
        );
        assert_eq!(
            program.step_interest(1),
            AttributeMask {
                flags: AttributeMask::ID,
                keys: 0b010
            }
        );
        assert_eq!(program.step_interest(3).flags, AttributeMask::ALL);
        assert_eq!(program.step_interest(3).keys, 0b001);
        assert_eq!(
            program.filter_interest(),
            AttributeMask {
                flags: 0,
                keys: 0b100
            }
        );
    }

    #[test]
    fn tag_lookup_agrees_with_name_lookup() {
        let queries = [Query::all("DIV p, my-widget, svg rect", Save::none())
            .unwrap()
            .build()];
        let program = Program::compile(&queries);

        for name in [
            "div",
            "Div",
            "p",
            "span",
            "my-widget",
            "MY-WIDGET",
            "svg",
            "rect",
            "x",
        ] {
            assert_eq!(
                program.tag_mask(TagId::of(name), name),
                program.name_mask(name),
                "{name}"
            );
        }
        assert!(program.tag_mask(TagId::of("span"), "span").is_none());
    }

    #[test]
    fn universal_and_sibling_steps_are_flagged() {
        let queries = [Query::first("h1 + .lead ~ *", Save::none())
            .unwrap()
            .build()];
        let program = Program::compile(&queries);

        assert_eq!(ones(program.mask(StepMask::AdjacentIn)), [1]);
        assert_eq!(ones(program.mask(StepMask::SiblingIn)), [2]);
        assert_eq!(ones(program.mask(StepMask::Universal)), [1, 2]);
        assert!(program.features().has_siblings);
        assert!(program.features().all_roots_first);
        assert!(program.features().parses_attributes);
    }

    #[test]
    fn collects_structural_requirements() {
        let queries = [
            Query::all("li:nth-of-type(2), :nth-child(odd of .x)", Save::none())
                .unwrap()
                .build(),
        ];
        let program = Program::compile(&queries);

        let features = program.features();
        assert!(features.has_structural);
        assert!(features.needs_type_ordinals);
        assert!(features.needs_filtered_ordinals);
        assert!(!features.needs_child_ordinals);
        assert_eq!(program.type_ordinal_names(), Some(vec!["li"]));
        assert_eq!(program.structural_filters().len(), 1);
    }

    #[test]
    fn steps_past_sixty_four_use_more_words() {
        let selector = vec!["div"; 70].join(" > ");
        let queries = [Query::all(&selector, Save::none()).unwrap().build()];
        let program = Program::compile(&queries);

        assert_eq!(program.words(), 2);
        assert_eq!(ones(program.mask(StepMask::Save)), [69]);
        assert_eq!(ones(program.name_mask("div").unwrap()).len(), 70);
    }
}
