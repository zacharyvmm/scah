//! Structural context for `:nth-child()` and related pseudo-classes.

use crate::XHtmlElement;
use scah_html::AttributeMask;
use scah_query_ir::{LocalSelectorList, Program, StructuralMatchContext};
use smallvec::SmallVec;

type TypeCounts<'html> = SmallVec<[(&'html str, u32); 4]>;

/// Sibling counters for structural pseudo-classes (`:nth-child`,
/// `:nth-of-type`, `:first-child`, ...), kept per open element.
pub(crate) struct StructuralState<'html, 'query> {
    child_counts: Option<Vec<u32>>,
    type_counts: Option<Vec<TypeCounts<'html>>>,
    tracked_type_names: Option<Vec<&'query str>>,
    filters: Vec<(&'query LocalSelectorList<'query>, Vec<u32>)>,
    /// Attributes the `of S` filters read.
    pub(crate) attribute_interest: AttributeMask,
    root_seen: bool,
}

impl<'html, 'query> StructuralState<'html, 'query> {
    pub(crate) fn new(program: &Program<'query>) -> Self {
        let features = program.features();
        let filters: Vec<_> = program
            .structural_filters()
            .into_iter()
            // Filter lists are evaluated without a structural context. The
            // selector parser rejects structural pseudo-classes inside `of S`,
            // but hand-built predicates can still contain them. Leaving such a
            // filter uncounted makes its ordinal fail closed instead of
            // counting siblings that do not match.
            .filter(|filter| {
                !filter
                    .as_slice()
                    .iter()
                    .any(|predicate| predicate.requires_structural())
            })
            .collect();
        Self {
            // Keep a virtual document parent so fragment roots participate in
            // ordinal selectors consistently.
            child_counts: features.needs_child_ordinals.then(|| vec![0]),
            type_counts: features.needs_type_ordinals.then(|| vec![SmallVec::new()]),
            tracked_type_names: features
                .needs_type_ordinals
                .then(|| program.type_ordinal_names())
                .flatten(),
            filters: filters
                .into_iter()
                .map(|filter| (filter, vec![0]))
                .collect(),
            attribute_interest: program.filter_interest(),
            root_seen: false,
        }
    }

    /// Count the element being opened among its siblings. `persistent`
    /// elements (not void) get counters for their own children.
    pub(crate) fn open(
        &mut self,
        name: &'html str,
        persistent: bool,
        element: &XHtmlElement<'html>,
    ) -> StructuralMatchContext<'query> {
        let is_document_root = !self.root_seen;
        self.root_seen = true;
        let child_index = self.child_counts.as_mut().map_or(0, |counts| {
            if let Some(count) = counts.last_mut() {
                *count = count.saturating_add(1);
                *count
            } else {
                1
            }
        });
        let tracks_type = self.tracked_type_names.as_ref().is_none_or(|names| {
            names
                .iter()
                .any(|tracked| tracked.eq_ignore_ascii_case(name))
        });
        let type_index = self.type_counts.as_mut().map_or(0, |levels| {
            if !tracks_type {
                return 0;
            }
            if let Some(counts) = levels.last_mut() {
                if let Some((_, count)) = counts
                    .iter_mut()
                    .find(|(child_name, _)| child_name.eq_ignore_ascii_case(name))
                {
                    *count = count.saturating_add(1);
                    *count
                } else {
                    counts.push((name, 1));
                    1
                }
            } else {
                1
            }
        });
        let mut filtered_child_indices = SmallVec::new();
        for (filter, counts) in &mut self.filters {
            if filter
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_local_element_unchecked(element))
            {
                let parent_count = counts.last_mut().expect("filter parent count");
                *parent_count = parent_count.saturating_add(1);
                filtered_child_indices.push((*filter, *parent_count));
            }
        }
        if persistent {
            if let Some(counts) = &mut self.child_counts {
                counts.push(0);
            }
            if let Some(counts) = &mut self.type_counts {
                counts.push(SmallVec::new());
            }
            for (_, counts) in &mut self.filters {
                counts.push(0);
            }
        }
        StructuralMatchContext {
            child_index,
            type_index,
            filtered_child_indices,
            is_document_root,
            is_scope_root: is_document_root,
        }
    }

    pub(crate) fn close(&mut self) {
        if let Some(counts) = &mut self.child_counts {
            counts.pop();
        }
        if let Some(counts) = &mut self.type_counts {
            counts.pop();
        }
        for (_, counts) in &mut self.filters {
            counts.pop();
        }
    }
}
