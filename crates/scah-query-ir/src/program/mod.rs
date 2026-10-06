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

use crate::query::compiler::{
    PredicateMetadata, QuerySection, QuerySectionId, QuerySpec, SelectionKind, TextRequirements,
    Transition, ascii_case_insensitive_hash,
};
use crate::query::selector::{
    Combinator, ElementPredicate, LocalLogicalPredicate, LocalSelectorList, StructuralPredicate,
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
}

const STEP_MASK_COUNT: usize = StepMask::Universal as usize + 1;

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
}

const SECTION_MASK_COUNT: usize = SectionMask::EntryChild as usize + 1;

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
    section: SectionIndex,
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
    features: ProgramFeatures,
}

impl<'q> Program<'q> {
    /// Compile every query of a parse into one program.
    ///
    /// Steps and sections are numbered in query order, so results keep the
    /// order of the `queries` slice.
    pub fn compile<Q: QuerySpec<'q>>(queries: &'q [Q]) -> Self {
        let step_count: usize = queries.iter().map(|query| query.states().len()).sum();
        let section_count: usize = queries.iter().map(|query| query.queries().len()).sum();
        let words = bits::words_for(step_count);
        let section_masks_start = STEP_MASK_COUNT * words;
        let name_masks_start = section_masks_start + SECTION_MASK_COUNT * words * section_count;

        let mut transitions: Vec<&'q Transition<'q>> = Vec::with_capacity(step_count);
        let mut step_sections = vec![SectionIndex(0); step_count];
        let mut masks = vec![0_u64; name_masks_start];
        let mut sections = Vec::with_capacity(section_count);
        let mut roots = Vec::new();
        let mut features = ProgramFeatures {
            all_roots_first: true,
            ..ProgramFeatures::default()
        };
        let step_bit = |masks: &mut [u64], mask: StepMask, step: usize| {
            bits::set(&mut masks[mask as usize * words..][..words], step);
        };
        let section_bit = |masks: &mut [u64], section: usize, mask: SectionMask, step: usize| {
            let start =
                section_masks_start + (section * SECTION_MASK_COUNT + mask as usize) * words;
            bits::set(&mut masks[start..start + words], step);
        };

        let mut step_offset = 0;
        let mut section_offset = 0;
        for query in queries {
            transitions.extend(query.states());
            for (local, section) in query.queries().iter().enumerate() {
                let index = section_offset + local;
                let parent = section
                    .parent
                    .map(|parent| SectionIndex((section_offset + parent.index()) as u32));
                sections.push(SectionInfo {
                    spec: section,
                    parent,
                    children: (0, 0),
                });
                if parent.is_none() {
                    roots.push(SectionIndex(index as u32));
                    features.all_roots_first &= section.kind == SelectionKind::First;
                } else {
                    features.has_scoped_sections = true;
                }
                features.text.raw_text |= section.save.raw_text;
                features.text.text |= section.save.text;
                features.stores_attributes |= section.save.attributes;

                let alternatives = query.selection_ranges(QuerySectionId(local));
                features.has_selector_lists |= alternatives.len() > 1;
                for alternative in alternatives {
                    let start = step_offset + alternative.start.index();
                    let end = step_offset + alternative.end.index();
                    for step in start..end {
                        step_sections[step] = SectionIndex(index as u32);
                        section_bit(&mut masks, index, SectionMask::Steps, step);
                        let transition = transitions[step];
                        let mask = if step == start {
                            match (parent.is_some(), &transition.guard) {
                                (false, Combinator::Descendant) => {
                                    Some(StepMask::RootEntryDescendant)
                                }
                                (false, Combinator::Child) => Some(StepMask::RootEntryChild),
                                (true, Combinator::Descendant) => {
                                    section_bit(
                                        &mut masks,
                                        index,
                                        SectionMask::EntryDescendant,
                                        step,
                                    );
                                    None
                                }
                                (true, Combinator::Child) => {
                                    section_bit(&mut masks, index, SectionMask::EntryChild, step);
                                    None
                                }
                                // A first step relative to the document or a
                                // scope element has no earlier sibling, and
                                // `|` is not modeled: such steps never match.
                                _ => None,
                            }
                        } else {
                            match &transition.guard {
                                Combinator::Descendant => Some(StepMask::DescendantIn),
                                Combinator::Child => Some(StepMask::ChildIn),
                                Combinator::NextSibling => {
                                    features.has_siblings = true;
                                    Some(StepMask::AdjacentIn)
                                }
                                Combinator::SubsequentSibling => {
                                    features.has_siblings = true;
                                    Some(StepMask::SiblingIn)
                                }
                                Combinator::Namespace => None,
                            }
                        };
                        if let Some(mask) = mask {
                            step_bit(&mut masks, mask, step);
                        }
                        if parent.is_some() {
                            step_bit(&mut masks, StepMask::Scoped, step);
                        }
                        if step + 1 == end {
                            step_bit(&mut masks, StepMask::Save, step);
                            section_bit(&mut masks, index, SectionMask::Save, step);
                            if section.save.attributes {
                                step_bit(&mut masks, StepMask::SaveAttributes, step);
                            }
                        }
                        let predicate = transition.predicate();
                        if !transition.metadata().local_name_only() {
                            step_bit(&mut masks, StepMask::Filter, step);
                        }
                        if predicate.name.is_none() {
                            step_bit(&mut masks, StepMask::Universal, step);
                        }
                        features.parses_attributes |= predicate.requires_attributes();
                        features.has_structural |= predicate.requires_structural();
                        visit_structural(predicate, &mut |structural| match structural {
                            StructuralPredicate::FirstChild | StructuralPredicate::NthChild(_) => {
                                features.needs_child_ordinals = true;
                            }
                            StructuralPredicate::FirstOfType
                            | StructuralPredicate::NthOfType(_) => {
                                features.needs_type_ordinals = true;
                            }
                            StructuralPredicate::NthChildOf(_, _) => {
                                features.needs_filtered_ordinals = true;
                            }
                            StructuralPredicate::Root | StructuralPredicate::Scope => {}
                        });
                    }
                }
            }
            step_offset += query.states().len();
            section_offset += query.queries().len();
        }
        features.all_roots_first &= !roots.is_empty();
        features.parses_attributes |= features.stores_attributes;

        // Section children, then the root sections, in one list.
        let mut links = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let start = links.len() as u32;
            links.extend(
                sections
                    .iter()
                    .enumerate()
                    .filter(|(_, section)| section.parent == Some(SectionIndex(index as u32)))
                    .map(|(child, _)| SectionIndex(child as u32)),
            );
            sections[index].children = (start, links.len() as u32);
        }
        let roots_start = links.len();
        links.extend(roots);

        // Type names, their masks, and their lookup keys.
        let mut strings: Vec<&'q str> = Vec::new();
        for (step, transition) in transitions.iter().enumerate() {
            let Some(name) = transition.predicate().name else {
                continue;
            };
            let entry = match strings
                .iter()
                .position(|existing| existing.eq_ignore_ascii_case(name))
            {
                Some(entry) => entry,
                None => {
                    strings.push(name);
                    masks.resize(masks.len() + words, 0);
                    strings.len() - 1
                }
            };
            let start = name_masks_start + entry * words;
            bits::set(&mut masks[start..start + words], step);
        }
        let mut packed = Vec::new();
        let mut hashed = Vec::new();
        for (entry, name) in strings.iter().enumerate() {
            match packed_name(name.as_bytes()) {
                Some(key) => packed.push((lowercase_packed(key), entry as u32)),
                None => hashed.push((ascii_case_insensitive_hash(name), entry as u32)),
            }
        }
        packed.sort_unstable();
        hashed.sort_unstable();
        let hashes_start = packed.len();
        packed.extend(hashed);
        let name_lengths = strings
            .iter()
            .fold(0, |lengths, name| lengths | (1 << name.len().min(63)));
        let mut tag_names = [NO_NAME; TagId::COUNT];
        for (entry, name) in strings.iter().enumerate() {
            let tag = TagId::of(name);
            if tag.is_known() {
                tag_names[tag.index()] = entry as u32;
            }
        }

        // Attribute names, shared by step and filter interest masks.
        let attributes_start = strings.len();
        let steps = transitions
            .iter()
            .zip(step_sections)
            .enumerate()
            .map(|(step, (transition, section))| {
                let mut interest =
                    attribute_mask(transition.metadata(), &mut strings, attributes_start);
                if bits::contains(&masks[StepMask::SaveAttributes as usize * words..], step) {
                    interest.flags |= AttributeMask::ALL;
                }
                StepInfo {
                    transition,
                    section,
                    interest,
                }
            })
            .collect();
        let filter_interest = structural_filters(&transitions)
            .iter()
            .flat_map(|filter| filter.as_slice())
            .fold(AttributeMask::default(), |mask, predicate| {
                mask.union(attribute_mask(
                    &PredicateMetadata::compile(predicate),
                    &mut strings,
                    attributes_start,
                ))
            });

        Self {
            words,
            steps,
            sections: sections.into_boxed_slice(),
            links: links.into_boxed_slice(),
            roots_start,
            masks: masks.into_boxed_slice(),
            section_masks_start,
            name_masks_start,
            strings: strings.into_boxed_slice(),
            attributes_start,
            name_keys: packed.into_boxed_slice(),
            hashes_start,
            name_lengths,
            tag_names,
            filter_interest,
            features,
        }
    }

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
        self.steps[step].section
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

/// Map predicate metadata onto the attribute names at `strings[start..]`,
/// adding names as needed.
fn attribute_mask<'q>(
    metadata: &PredicateMetadata<'q>,
    strings: &mut Vec<&'q str>,
    start: usize,
) -> AttributeMask {
    let mut mask = AttributeMask::default();
    if metadata.needs_id() {
        mask.flags |= AttributeMask::ID;
    }
    if metadata.needs_class() {
        mask.flags |= AttributeMask::CLASS;
    }
    for &name in metadata.attribute_names() {
        let index = match strings[start..]
            .iter()
            .position(|known| known.eq_ignore_ascii_case(name))
        {
            Some(index) => index,
            None => {
                strings.push(name);
                strings.len() - start - 1
            }
        };
        if index < 64 {
            mask.keys |= 1 << index;
        } else {
            mask.flags |= AttributeMask::ALL;
        }
    }
    mask
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
