//! [`Program::compile`], as a sequence of passes over the program's tables.

use super::{
    AttributeMask, LaneSource, NO_NAME, Program, ProgramFeatures, SECTION_MASK_COUNT,
    STEP_MASK_COUNT, SectionIndex, SectionInfo, SectionMask, StepInfo, StepMask, StepPlan, bits,
    lowercase_packed, packed_name, structural_filters, visit_structural,
};
use crate::query::compiler::{
    PredicateMetadata, QuerySection, QuerySectionId, QuerySpec, SelectionKind, Transition,
    ascii_case_insensitive_hash,
};
use crate::query::selector::{Combinator, StructuralPredicate};
use scah_html::TagId;
use std::ops::Range;

impl<'q> Program<'q> {
    /// Compile every query of a parse into one program.
    ///
    /// Steps and sections are numbered in query order, so results keep the
    /// order of the `queries` slice.
    pub fn compile<Q: QuerySpec<'q>>(queries: &'q [Q]) -> Self {
        let mut tables = Tables::new(queries);
        for query in queries {
            tables.add_query(query);
        }
        tables.finish_features();
        tables.mark_read_steps();
        tables.link_sections();
        tables.mark_child_entries();
        let names = TypeNames::index(&tables.transitions, &mut tables.masks, tables.layout);
        tables.finish(names)
    }
}

/// Where each kind of mask starts in the mask arena.
#[derive(Debug, Clone, Copy)]
struct Layout {
    words: usize,
    section_masks_start: usize,
    name_masks_start: usize,
}

impl Layout {
    fn step_row(self, mask: StepMask) -> Range<usize> {
        let start = mask as usize * self.words;
        start..start + self.words
    }

    fn section_row(self, section: usize, mask: SectionMask) -> Range<usize> {
        let start =
            self.section_masks_start + (section * SECTION_MASK_COUNT + mask as usize) * self.words;
        start..start + self.words
    }

    fn name_row(self, entry: usize) -> Range<usize> {
        let start = self.name_masks_start + entry * self.words;
        start..start + self.words
    }
}

/// The program's tables while they are being built.
struct Tables<'q> {
    layout: Layout,
    transitions: Vec<&'q Transition<'q>>,
    step_sections: Vec<SectionIndex>,
    masks: Vec<u64>,
    sections: Vec<SectionInfo<'q>>,
    roots: Vec<SectionIndex>,
    /// Every section's children, then the root sections.
    links: Vec<SectionIndex>,
    roots_start: usize,
    claim_words: usize,
    features: ProgramFeatures,
}

impl<'q> Tables<'q> {
    fn new<Q: QuerySpec<'q>>(queries: &'q [Q]) -> Self {
        let step_count: usize = queries.iter().map(|query| query.states().len()).sum();
        let section_count: usize = queries.iter().map(|query| query.queries().len()).sum();
        let words = bits::words_for(step_count);
        let section_masks_start = STEP_MASK_COUNT * words;
        let name_masks_start = section_masks_start + SECTION_MASK_COUNT * words * section_count;
        Self {
            layout: Layout {
                words,
                section_masks_start,
                name_masks_start,
            },
            transitions: Vec::with_capacity(step_count),
            step_sections: Vec::with_capacity(step_count),
            masks: vec![0; name_masks_start],
            sections: Vec::with_capacity(section_count),
            roots: Vec::new(),
            links: Vec::new(),
            roots_start: 0,
            claim_words: 1,
            features: ProgramFeatures {
                all_roots_first: true,
                ..ProgramFeatures::default()
            },
        }
    }

    fn set_step(&mut self, mask: StepMask, step: usize) {
        bits::set(&mut self.masks[self.layout.step_row(mask)], step);
    }

    fn set_section(&mut self, section: usize, mask: SectionMask, step: usize) {
        bits::set(
            &mut self.masks[self.layout.section_row(section, mask)],
            step,
        );
    }

    /// Number the steps and sections of `query`, and mark each step in the
    /// masks that follow from its own selector: how it is entered, whether it
    /// saves, and whether it needs more than a name check.
    fn add_query<Q: QuerySpec<'q>>(&mut self, query: &'q Q) {
        let step_offset = self.transitions.len();
        let section_offset = self.sections.len();
        self.transitions.extend(query.states());
        self.step_sections
            .resize(self.transitions.len(), SectionIndex(0));
        for (local, section) in query.queries().iter().enumerate() {
            let index = section_offset + local;
            let parent = section
                .parent
                .map(|parent| SectionIndex((section_offset + parent.index()) as u32));
            self.add_section(index, section, parent);
            let alternatives = query.selection_ranges(QuerySectionId(local));
            self.features.has_selector_lists |= alternatives.len() > 1;
            for alternative in alternatives {
                let steps =
                    step_offset + alternative.start.index()..step_offset + alternative.end.index();
                for step in steps.clone() {
                    self.add_step(step, index, parent.is_some(), steps.clone(), section);
                }
            }
        }
    }

    /// Features that depend on every section.
    fn finish_features(&mut self) {
        self.features.all_roots_first &= !self.roots.is_empty();
        self.features.parses_attributes |= self.features.stores_attributes;
    }

    fn add_section(
        &mut self,
        index: usize,
        section: &'q QuerySection<'q>,
        parent: Option<SectionIndex>,
    ) {
        self.sections.push(SectionInfo {
            spec: section,
            parent,
            children: (0, 0),
        });
        if parent.is_none() {
            self.roots.push(SectionIndex(index as u32));
            self.features.all_roots_first &= section.kind == SelectionKind::First;
        } else {
            self.features.has_scoped_sections = true;
        }
        self.features.text.raw_text |= section.save.raw_text;
        self.features.text.text |= section.save.text;
        self.features.stores_attributes |= section.save.attributes;
    }

    /// Mark `step`, one step of the alternative `alternative` of `section`.
    fn add_step(
        &mut self,
        step: usize,
        section: usize,
        nested: bool,
        alternative: Range<usize>,
        spec: &QuerySection<'q>,
    ) {
        self.step_sections[step] = SectionIndex(section as u32);
        self.set_section(section, SectionMask::Steps, step);
        let transition = self.transitions[step];
        let incoming = if step == alternative.start {
            match (nested, &transition.guard) {
                (false, Combinator::Descendant) => Some(StepMask::RootEntryDescendant),
                (false, Combinator::Child) => Some(StepMask::RootEntryChild),
                (true, Combinator::Descendant) => {
                    self.set_section(section, SectionMask::EntryDescendant, step);
                    None
                }
                (true, Combinator::Child) => {
                    self.set_section(section, SectionMask::EntryChild, step);
                    None
                }
                // A first step relative to the document or a scope element
                // has no earlier sibling, and `|` is not modeled: such steps
                // never match.
                _ => None,
            }
        } else {
            match &transition.guard {
                Combinator::Descendant => Some(StepMask::DescendantIn),
                Combinator::Child => Some(StepMask::ChildIn),
                Combinator::NextSibling => {
                    self.features.has_siblings = true;
                    Some(StepMask::AdjacentIn)
                }
                Combinator::SubsequentSibling => {
                    self.features.has_siblings = true;
                    Some(StepMask::SiblingIn)
                }
                Combinator::Namespace => None,
            }
        };
        if let Some(mask) = incoming {
            self.set_step(mask, step);
        }
        if nested {
            self.set_step(StepMask::Scoped, step);
        }
        if step + 1 == alternative.end {
            self.set_step(StepMask::Save, step);
            self.set_section(section, SectionMask::Save, step);
            if spec.save.attributes {
                self.set_step(StepMask::SaveAttributes, step);
            }
        }

        let predicate = transition.predicate();
        if !transition.metadata().local_name_only() {
            self.set_step(StepMask::Filter, step);
        }
        if predicate.name.is_none() {
            self.set_step(StepMask::Universal, step);
        }
        self.features.parses_attributes |= predicate.requires_attributes();
        self.features.has_structural |= predicate.requires_structural();
        let features = &mut self.features;
        visit_structural(predicate, &mut |structural| match structural {
            StructuralPredicate::FirstChild | StructuralPredicate::NthChild(_) => {
                features.needs_child_ordinals = true;
            }
            StructuralPredicate::FirstOfType | StructuralPredicate::NthOfType(_) => {
                features.needs_type_ordinals = true;
            }
            StructuralPredicate::NthChildOf(_, _) => {
                features.needs_filtered_ordinals = true;
            }
            StructuralPredicate::Root | StructuralPredicate::Scope => {}
        });
    }

    /// Mark the steps a later step reads, from the incoming-combinator masks:
    /// a step followed by `>`, `+` or `~` is read by a child or sibling, and
    /// one followed by ` ` by descendants.
    fn mark_read_steps(&mut self) {
        let layout = self.layout;
        let masks = &mut self.masks;
        for word in 0..layout.words {
            let read_by = |masks: &[u64], mask: StepMask| {
                let mask = &masks[layout.step_row(mask)];
                (mask[word] >> 1) | (mask.get(word + 1).copied().unwrap_or(0) << 63)
            };
            let child_read = read_by(masks, StepMask::ChildIn)
                | read_by(masks, StepMask::AdjacentIn)
                | read_by(masks, StepMask::SiblingIn);
            let descendant_read = read_by(masks, StepMask::DescendantIn);
            masks[layout.step_row(StepMask::ChildRead)][word] = child_read;
            masks[layout.step_row(StepMask::DescendantRead)][word] = descendant_read;
            masks[layout.step_row(StepMask::RedundantIfInherited)][word] = descendant_read
                & !child_read
                & !masks[layout.step_row(StepMask::Save)][word]
                & !masks[layout.step_row(StepMask::Scoped)][word];
        }
    }

    /// List each section's children, then the root sections, in one list.
    fn link_sections(&mut self) {
        self.links.reserve(self.sections.len() + self.roots.len());
        for index in 0..self.sections.len() {
            let start = self.links.len() as u32;
            let parent = Some(SectionIndex(index as u32));
            for (child, section) in self.sections.iter().enumerate() {
                if section.parent == parent {
                    self.links.push(SectionIndex(child as u32));
                }
            }
            self.sections[index].children = (start, self.links.len() as u32);
            let children = (self.links.len() as u32 - start) as usize;
            self.claim_words = self.claim_words.max(bits::words_for(children));
        }
        self.roots_start = self.links.len();
        self.links.append(&mut self.roots);
    }

    /// What each section's open scopes enable: its children's entry steps.
    fn mark_child_entries(&mut self) {
        let layout = self.layout;
        for (section, info) in self.sections.iter().enumerate() {
            let (start, end) = info.children;
            for &child in &self.links[start as usize..end as usize] {
                for (entry, union) in [
                    (
                        SectionMask::EntryDescendant,
                        SectionMask::ChildEntryDescendant,
                    ),
                    (SectionMask::EntryChild, SectionMask::ChildEntryChild),
                ] {
                    let source = layout.section_row(child.index(), entry);
                    let target = layout.section_row(section, union);
                    for word in 0..layout.words {
                        self.masks[target.start + word] |= self.masks[source.start + word];
                    }
                }
            }
        }
    }

    /// Plan each step and assemble the program. Attribute names are added
    /// after the type names, as the steps and filters ask for them.
    fn finish(self, names: TypeNames<'q>) -> Program<'q> {
        let TypeNames {
            mut strings,
            keys,
            hashes_start,
            lengths,
            tags,
        } = names;
        let layout = self.layout;
        let attributes_start = strings.len();
        let mut lane_count = 0;
        let steps = (0..self.transitions.len())
            .map(|step| {
                let transition = self.transitions[step];
                let mut interest =
                    attribute_mask(transition.metadata(), &mut strings, attributes_start);
                if bits::contains(&self.masks[layout.step_row(StepMask::SaveAttributes)], step) {
                    interest.flags |= AttributeMask::ALL;
                }
                let lane = self.lane(step, &mut lane_count);
                StepInfo {
                    transition,
                    plan: self.plan(step, lane),
                    interest,
                }
            })
            .collect();
        let filter_interest = structural_filters(&self.transitions)
            .iter()
            .flat_map(|filter| filter.as_slice())
            .fold(AttributeMask::default(), |mask, predicate| {
                mask.union(attribute_mask(
                    &PredicateMetadata::compile(predicate),
                    &mut strings,
                    attributes_start,
                ))
            });

        Program {
            words: layout.words,
            steps,
            sections: self.sections.into_boxed_slice(),
            links: self.links.into_boxed_slice(),
            roots_start: self.roots_start,
            masks: self.masks.into_boxed_slice(),
            section_masks_start: layout.section_masks_start,
            name_masks_start: layout.name_masks_start,
            strings: strings.into_boxed_slice(),
            attributes_start,
            name_keys: keys.into_boxed_slice(),
            hashes_start,
            name_lengths: lengths,
            tag_names: tags,
            filter_interest,
            lane_count,
            claim_words: self.claim_words,
            features: self.features,
        }
    }

    /// The lane of `step` and where it comes from: nested-section steps get
    /// the next lane, root-section steps none.
    fn lane(&self, step: usize, lane_count: &mut usize) -> (u32, LaneSource) {
        let section = self.step_sections[step];
        if self.sections[section.index()].parent.is_none() {
            return (u32::MAX, LaneSource::Never);
        }
        let layout = self.layout;
        let in_step_mask =
            |mask: StepMask| bits::contains(&self.masks[layout.step_row(mask)], step);
        let in_section_mask = |mask: SectionMask| {
            bits::contains(&self.masks[layout.section_row(section.index(), mask)], step)
        };
        let source = if in_section_mask(SectionMask::EntryDescendant) {
            LaneSource::EntryDescendant
        } else if in_section_mask(SectionMask::EntryChild) {
            LaneSource::EntryChild
        } else if in_step_mask(StepMask::DescendantIn) {
            LaneSource::Descendant
        } else if in_step_mask(StepMask::ChildIn) {
            LaneSource::Child
        } else if in_step_mask(StepMask::AdjacentIn) {
            LaneSource::Adjacent
        } else if in_step_mask(StepMask::SiblingIn) {
            LaneSource::Sibling
        } else {
            LaneSource::Never
        };
        *lane_count += 1;
        (*lane_count as u32 - 1, source)
    }

    fn plan(&self, step: usize, (lane, lane_source): (u32, LaneSource)) -> StepPlan {
        let section = self.step_sections[step];
        let info = &self.sections[section.index()];
        let child = info.parent.map_or(0, |parent| {
            let (start, end) = self.sections[parent.index()].children;
            self.links[start as usize..end as usize]
                .iter()
                .position(|&child| child == section)
                .expect("nested section is a child of its parent") as u32
        });
        StepPlan {
            section,
            lane,
            lane_source,
            child,
            root: info.parent.is_none(),
            first: info.spec.kind == SelectionKind::First,
            has_children: info.children.0 != info.children.1,
        }
    }
}

/// Type-selector names, and how to find a tag's entry among them.
struct TypeNames<'q> {
    /// Distinct names, compared ASCII case-insensitively.
    strings: Vec<&'q str>,
    /// Packed keys of short names, sorted, then hashes of long ones; each
    /// with its index into `strings`.
    keys: Vec<(u64, u32)>,
    hashes_start: usize,
    /// Bit `n` is set when some name is `n` bytes long.
    lengths: u64,
    /// Entry of each known tag, or [`NO_NAME`].
    tags: [u32; TagId::COUNT],
}

impl<'q> TypeNames<'q> {
    /// Collect the type names of `transitions`, appending one step mask per
    /// name to `masks`.
    fn index(transitions: &[&'q Transition<'q>], masks: &mut Vec<u64>, layout: Layout) -> Self {
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
                    masks.resize(masks.len() + layout.words, 0);
                    strings.len() - 1
                }
            };
            bits::set(&mut masks[layout.name_row(entry)], step);
        }

        let mut keys = Vec::new();
        let mut hashed = Vec::new();
        for (entry, name) in strings.iter().enumerate() {
            match packed_name(name.as_bytes()) {
                Some(key) => keys.push((lowercase_packed(key), entry as u32)),
                None => hashed.push((ascii_case_insensitive_hash(name), entry as u32)),
            }
        }
        keys.sort_unstable();
        hashed.sort_unstable();
        let hashes_start = keys.len();
        keys.extend(hashed);
        let lengths = strings
            .iter()
            .fold(0, |lengths, name| lengths | (1 << name.len().min(63)));
        let mut tags = [NO_NAME; TagId::COUNT];
        for (entry, name) in strings.iter().enumerate() {
            let tag = TagId::of(name);
            if tag.is_known() {
                tags[tag.index()] = entry as u32;
            }
        }
        Self {
            strings,
            keys,
            hashes_start,
            lengths,
            tags,
        }
    }
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
