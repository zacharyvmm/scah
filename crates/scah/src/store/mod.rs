use crate::Attribute;
use crate::QuerySection;
use scah_query_ir::{Program, SectionIndex};
use std::ops::Range;

mod text;
pub(crate) use text::{TextStore, TextTape, trim_collapsed_range};
mod arena;
mod attributes;
mod element;

pub use arena::{
    Arena,
    id::{AttributeId, ElementId},
};

pub use element::Element;
pub(crate) use element::ElementTextRanges;

/// A query section as the store sees it: enough to resolve lookups.
#[derive(Debug, Clone, PartialEq)]
struct StoreSection<'query> {
    selector: &'query str,
    parent: Option<u32>,
    /// Index of the query this section belongs to.
    query: u32,
}

/// Parent group of results saved directly under the document.
const DOCUMENT: u32 = 0;

/// The result set returned by [`parse`](crate::parse).
///
/// A `Store` holds flat tables: one [`Element`] row per element and query
/// section that saved it, the attributes and text those rows captured, and
/// an index from each parent (the document, or a row of the parent section)
/// to its results, grouped by section in document order. Look results up by
/// selector string with [`Store::get`], or by position with
/// [`Store::query`].
///
/// # Example
///
/// ```rust
/// use scah::{Query, Save, parse};
///
/// let html = "<div><a href='x'>Link1</a><a href='y'>Link2</a></div>";
/// let queries = &[Query::all("a", Save::all())
///     .expect("valid selector")
///     .build()];
/// let store = parse(html, queries).expect("parse succeeds");
///
/// // Retrieve all matched <a> elements
/// let anchors: Vec<_> = store.get("a").unwrap().collect();
/// assert_eq!(anchors.len(), 2);
///
/// // Access attributes
/// assert_eq!(anchors[0].attribute(&store, "href"), Some("x"));
/// ```
#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    /// One row per element and section that saved it. An element saved by a
    /// nested section under several parent matches has one row, listed
    /// under each parent.
    pub elements: Arena<Element<'html>, ElementId>,
    /// Arena of attributes belonging to matched elements.
    pub attributes: Arena<Attribute<'html>, AttributeId>,
    /// Accumulated raw-text and normalized-text buffers shared by all elements.
    pub(crate) text: TextStore,
    /// Lazily allocated sidecar for raw/normalized text ranges.
    ///
    /// Remains unallocated (`None`) when no query requests text capture, so
    /// inner-HTML-only / no-content workloads do not pay per-element text
    /// range storage on [`Element`].
    element_text_ranges: Option<Box<ElementTextRanges>>,
    /// Sections of the parsed queries, in declaration order.
    sections: Box<[StoreSection<'query>]>,
    /// Section of each row.
    row_sections: Vec<u32>,
    /// `(parent group, row)` per save, in document order. Group 0 is the
    /// document; group `r + 1` is row `r`. Not recorded when every section
    /// is a root: then every row is a document result.
    edges: Vec<(u32, u32)>,
    record_edges: bool,
    /// Results of group `g` are `results[offsets[g]..offsets[g + 1]]`,
    /// sorted by section, then document order. Built by [`Store::finish`].
    offsets: Vec<u32>,
    results: Vec<ElementId>,
    #[cfg(any(debug_assertions, test))]
    pub trace: crate::debug::TraceStore<'html, 'query>,
}

/// Advanced allocation tuning for [`Store::with_capacity_options`].
///
/// Controls how the `Store` pre-allocates arena capacity from a total
/// HTML byte-length hint. The defaults are the optimized parser/store
/// heuristics used by [`Store::with_capacity`] and are suitable for the
/// vast majority of workloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityOptions {
    /// Approximate HTML bytes per reserved element slot.
    ///
    /// Default: 48 (derived from valgrind massif profiling).
    pub element_bytes_per_slot: usize,

    /// Approximate HTML bytes per reserved attribute slot.
    ///
    /// Default: 24.
    pub attribute_bytes_per_slot: usize,

    /// Whether to reserve the raw-text buffer using the full input capacity.
    /// Set to `false` when no query needs raw text.
    ///
    /// Default: `true`, preserving the public [`Store::with_capacity`]
    /// behaviour of reserving raw-text storage.
    pub reserve_raw_text: bool,

    /// Whether to reserve the normalized-text buffer using the full input
    /// capacity. Set to `false` when no query needs normalized text.
    ///
    /// Default: `true`, preserving the public [`Store::with_capacity`]
    /// behaviour of reserving normalized-text storage.
    pub reserve_text: bool,

    /// Maximum trace-log preallocation.  Only used behind
    /// `cfg(any(debug_assertions, test))`; harmless otherwise.
    ///
    /// Default: 4096.
    pub trace_capacity_limit: usize,
}

impl Default for CapacityOptions {
    fn default() -> Self {
        Self {
            element_bytes_per_slot: 48,
            attribute_bytes_per_slot: 24,
            reserve_raw_text: true,
            reserve_text: true,
            trace_capacity_limit: 4096,
        }
    }
}

impl<'html, 'query: 'html> Default for Store<'html, 'query> {
    fn default() -> Self {
        Self {
            elements: Arena::new(),
            text: TextStore::new(),
            attributes: Arena::new(),
            element_text_ranges: None,
            sections: Box::default(),
            row_sections: Vec::new(),
            edges: Vec::new(),
            record_edges: true,
            offsets: Vec::new(),
            results: Vec::new(),
            #[cfg(any(debug_assertions, test))]
            trace: crate::debug::TraceStore::new(),
        }
    }
}

impl<'html, 'query: 'html> Store<'html, 'query> {
    /// Creates a `Store` with pre-allocated capacity for the arenas.
    ///
    /// The `capacity` parameter is the total HTML byte length. From this we
    /// derive conservative reservations for the element and attribute arenas
    /// using the default [`CapacityOptions`].
    ///
    /// By default this reserves capacity for **both** text representations
    /// (`raw_text` and normalized `text`). Query-aware [`crate::parse`]
    /// construction reserves only the representations required by the
    /// supplied queries via [`CapacityOptions`].
    ///
    /// This is the non-breaking public API. For advanced tuning (e.g.
    /// skipping text-content reservation or adjusting element/attribute
    /// ratios), use [`Store::with_capacity_options`].
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_options(capacity, CapacityOptions::default())
    }

    /// Creates a `Store` with pre-allocated capacity controlled by
    /// [`CapacityOptions`].
    ///
    /// # Allocation policy
    ///
    /// | Arena        | Reservation                        |
    /// |------------- |----------------------------------- |
    /// | elements     | `capacity / element_bytes_per_slot` |
    /// | attributes   | `capacity / attribute_bytes_per_slot`|
    /// | raw_text     | `capacity` if `reserve_raw_text`     |
    /// | text         | `capacity` if `reserve_text`         |
    /// | queries      | none (fixed small per-query alloc)   |
    ///
    /// Every divisor is clamped to at least 1 to avoid division by zero.
    pub fn with_capacity_options(capacity: usize, options: CapacityOptions) -> Self {
        Self::with_capacity_requirements(capacity, options, true, true)
    }

    pub(crate) fn with_capacity_requirements(
        capacity: usize,
        options: CapacityOptions,
        reserve_attributes: bool,
        reserve_text_buffers: bool,
    ) -> Self {
        let element_divisor = options.element_bytes_per_slot.max(1);
        let attribute_divisor = options.attribute_bytes_per_slot.max(1);
        let element_slots = capacity / element_divisor;

        Self {
            elements: Arena::with_capacity(element_slots),
            text: TextStore::with_capacity(
                if reserve_text_buffers && options.reserve_raw_text {
                    capacity
                } else {
                    0
                },
                if reserve_text_buffers && options.reserve_text {
                    capacity
                } else {
                    0
                },
            ),
            attributes: if reserve_attributes {
                Arena::with_capacity(capacity / attribute_divisor)
            } else {
                Arena::new()
            },
            element_text_ranges: None,
            sections: Box::default(),
            row_sections: Vec::with_capacity(element_slots),
            edges: Vec::new(),
            record_edges: true,
            offsets: Vec::new(),
            results: Vec::new(),
            #[cfg(any(debug_assertions, test))]
            trace: crate::debug::TraceStore::with_capacity(
                element_slots.min(options.trace_capacity_limit),
            ),
        }
    }

    #[inline(always)]
    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    pub(crate) fn trace_event(
        &mut self,
        #[cfg(any(debug_assertions, test))] event: crate::debug::TraceEvent<'html, 'query>,
    ) {
        #[cfg(any(debug_assertions, test))]
        {
            self.trace.push(event);
        }
    }

    /// Results saved for the first query whose selector is `selector`.
    ///
    /// The `selector` must be the **exact same string** used to build the
    /// [`Query`](crate::Query) (e.g. `"main > section > a[href]"`). When
    /// several queries share a selector, use [`Store::query`] to pick one.
    ///
    /// Returns `None` when no query has that selector or it matched nothing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scah::{Query, Save, parse};
    ///
    /// let html = "<ul><li>A</li><li>B</li></ul>";
    /// let queries = &[Query::all("li", Save::only_text())
    ///     .expect("valid selector")
    ///     .build()];
    /// let store = parse(html, queries).expect("parse succeeds");
    ///
    /// for li in store.get("li").unwrap() {
    ///     println!("{}", li.text(&store).unwrap_or_default());
    /// }
    /// ```
    pub fn get(&self, selector: &str) -> Option<impl Iterator<Item = &Element<'html>>> {
        self.elements_of(self.results(selector)?)
    }

    /// Results saved for the query at `index` in the slice given to
    /// [`parse`](crate::parse).
    ///
    /// Returns `None` when there is no such query or it matched nothing.
    pub fn query(&self, index: usize) -> Option<impl Iterator<Item = &Element<'html>>> {
        self.elements_of(self.query_results(index)?)
    }

    /// Row ids of [`Store::get`]'s results.
    pub fn results(&self, selector: &str) -> Option<&[ElementId]> {
        let section = self
            .sections
            .iter()
            .position(|section| section.parent.is_none() && section.selector == selector)?;
        self.section_results(DOCUMENT, section as u32)
    }

    /// Row ids of [`Store::query`]'s results.
    pub fn query_results(&self, index: usize) -> Option<&[ElementId]> {
        let section = self
            .sections
            .iter()
            .position(|section| section.parent.is_none() && section.query as usize == index)?;
        self.section_results(DOCUMENT, section as u32)
    }

    /// Row ids saved under `parent` by its first nested section whose
    /// selector is `selector`.
    pub fn child_results(&self, parent: ElementId, selector: &str) -> Option<&[ElementId]> {
        let section = self.row_sections[parent.index()];
        let child = self.sections.iter().position(|candidate| {
            candidate.parent == Some(section) && candidate.selector == selector
        })?;
        self.section_results(parent.index() as u32 + 1, child as u32)
    }

    /// Row ids saved under `parent` by its nested section at `index`, in
    /// the order the sections were declared.
    pub fn nested_results(&self, parent: ElementId, index: usize) -> Option<&[ElementId]> {
        let section = self.row_sections[parent.index()];
        let child = self
            .sections
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.parent == Some(section))
            .nth(index)?
            .0;
        self.section_results(parent.index() as u32 + 1, child as u32)
    }

    /// Number of results across all parents; a row listed under several
    /// parents counts once per parent.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Selector of the query at `index`.
    pub fn query_selector(&self, index: usize) -> Option<&'query str> {
        self.sections
            .iter()
            .find(|section| section.parent.is_none() && section.query as usize == index)
            .map(|section| section.selector)
    }

    /// Selector of `parent`'s nested section at `index`.
    pub fn nested_selector(&self, parent: ElementId, index: usize) -> Option<&'query str> {
        let section = self.row_sections[parent.index()];
        self.sections
            .iter()
            .filter(|candidate| candidate.parent == Some(section))
            .nth(index)
            .map(|section| section.selector)
    }

    fn elements_of(&self, ids: &[ElementId]) -> Option<impl Iterator<Item = &Element<'html>>> {
        Some(ids.iter().map(|&id| &self.elements[id]))
    }

    /// Results of `section` in parent group `group`, or `None` when empty.
    fn section_results(&self, group: u32, section: u32) -> Option<&[ElementId]> {
        let start = *self.offsets.get(group as usize)? as usize;
        let end = *self.offsets.get(group as usize + 1)? as usize;
        let group = &self.results[start..end];
        let first = group.partition_point(|row| self.row_sections[row.index()] < section);
        let last = group.partition_point(|row| self.row_sections[row.index()] <= section);
        (first < last).then(|| &group[first..last])
    }

    /// Record the sections of the queries being parsed.
    pub(crate) fn set_sections(&mut self, program: &Program<'query>) {
        let mut query = 0;
        self.sections = (0..program.section_count())
            .map(|index| {
                let section = SectionIndex(index as u32);
                let parent = program.section_parent(section);
                if parent.is_none() && index > 0 {
                    query += 1;
                }
                StoreSection {
                    selector: program.section(section).source,
                    parent: parent.map(|parent| parent.0),
                    query,
                }
            })
            .collect();
        self.record_edges = program.features().has_scoped_sections;
    }

    /// Save `element` as a new row of `section`.
    ///
    /// The row is not a result until [`Store::add_edge`] lists it under a
    /// parent.
    pub(crate) fn add_row(
        &mut self,
        section: SectionIndex,
        spec: &QuerySection<'query>,
        element: &crate::XHtmlElement<'html>,
    ) -> ElementId {
        let attributes = spec.save.attributes;
        let row = ElementId::from(self.elements.len());
        self.elements.push(Element {
            name: element.name,
            class: if attributes { element.class } else { None },
            id: if attributes { element.id } else { None },
            attributes: if attributes {
                self.attributes.attribute_slice_to_range(element.attributes)
            } else {
                None
            },
            inner_html: None,
        });
        self.row_sections.push(section.0);
        row
    }

    /// List `row` among the results of `parent` (the document when `None`).
    #[inline]
    pub(crate) fn add_edge(&mut self, parent: Option<ElementId>, row: ElementId) {
        if self.record_edges {
            let group = parent.map_or(DOCUMENT, |parent| parent.0 + 1);
            self.edges.push((group, row.0));
        } else {
            debug_assert!(
                parent.is_none(),
                "root-only stores list rows under the document"
            );
        }
    }

    /// Group the saved edges by parent, then section, keeping document order
    /// within each group (two stable counting sorts).
    pub(crate) fn finish(&mut self) {
        if !self.offsets.is_empty() {
            return;
        }
        if !self.record_edges {
            self.finish_roots();
            return;
        }
        let groups = self.elements.len() + 1;
        self.offsets.resize(groups + 1, 0);
        for &(group, _) in &self.edges {
            self.offsets[group as usize + 1] += 1;
        }
        for index in 1..self.offsets.len() {
            self.offsets[index] += self.offsets[index - 1];
        }

        // Edges usually arrive grouped by parent and section already: then
        // the results are the edges in order.
        let key = |&(group, row): &(u32, u32)| (group, self.row_sections[row as usize]);
        let edges = std::mem::take(&mut self.edges);
        if edges.windows(2).all(|pair| key(&pair[0]) <= key(&pair[1])) {
            self.results
                .extend(edges.iter().map(|&(_, row)| ElementId(row)));
            return;
        }

        // Otherwise sort by section, then stably by parent group.
        let mut by_section = vec![(0_u32, 0_u32); edges.len()];
        let mut next = self.section_starts(edges.iter().map(|&(_, row)| row));
        for &edge in &edges {
            let slot = &mut next[self.row_sections[edge.1 as usize] as usize];
            by_section[*slot as usize] = edge;
            *slot += 1;
        }
        let mut next = self.offsets.clone();
        self.results.resize(by_section.len(), ElementId(0));
        for (group, row) in by_section {
            let slot = &mut next[group as usize];
            self.results[*slot as usize] = ElementId(row);
            *slot += 1;
        }
    }

    /// Every row is a document result: group them by section.
    fn finish_roots(&mut self) {
        let rows = self.elements.len() as u32;
        self.offsets.extend([0, rows]);
        if self.row_sections.windows(2).all(|pair| pair[0] <= pair[1]) {
            self.results.extend((0..rows).map(ElementId));
            return;
        }
        let mut next = self.section_starts(0..rows);
        self.results.resize(rows as usize, ElementId(0));
        for row in 0..rows {
            let slot = &mut next[self.row_sections[row as usize] as usize];
            self.results[*slot as usize] = ElementId(row);
            *slot += 1;
        }
    }

    /// Start of each section's run when `rows` are sorted by section.
    fn section_starts(&self, rows: impl Iterator<Item = u32>) -> Vec<u32> {
        let mut starts = vec![0_u32; self.sections.len().max(1) + 1];
        for row in rows {
            starts[self.row_sections[row as usize] as usize + 1] += 1;
        }
        for index in 1..starts.len() {
            starts[index] += starts[index - 1];
        }
        starts
    }

    pub fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    ) {
        assert!(!self.elements.is_empty());
        assert!(element_id.index() < self.elements.len());

        #[cfg(any(debug_assertions, test))]
        let tag = self.elements[element_id].name;
        #[cfg(any(debug_assertions, test))]
        let has_inner_html = inner_html.is_some();
        #[cfg(any(debug_assertions, test))]
        let has_raw_text = raw_text.is_some();
        #[cfg(any(debug_assertions, test))]
        let has_text = text.is_some();

        self.elements[element_id].inner_html = inner_html;
        if let Some(range) = raw_text {
            self.element_text_ranges
                .get_or_insert_with(Default::default)
                .set_raw_text(element_id, range);
        }
        if let Some(range) = text {
            self.element_text_ranges
                .get_or_insert_with(Default::default)
                .set_text(element_id, range);
        }

        crate::scah_trace!(
            self,
            crate::debug::TraceEvent::ContentFinalized {
                element_id,
                tag,
                has_inner_html,
                has_raw_text,
                has_text,
            }
        );
    }

    #[inline]
    pub(crate) fn raw_text_range(&self, element_id: ElementId) -> Option<&Range<usize>> {
        self.element_text_ranges
            .as_ref()
            .and_then(|ranges| ranges.raw_text(element_id))
    }

    #[inline]
    pub(crate) fn text_range(&self, element_id: ElementId) -> Option<&Range<usize>> {
        self.element_text_ranges
            .as_ref()
            .and_then(|ranges| ranges.text(element_id))
    }

    #[cfg(test)]
    pub(crate) fn tracks_element_text_ranges(&self) -> bool {
        self.element_text_ranges.is_some()
    }

    #[cfg(test)]
    pub(crate) fn tracks_raw_text_ranges(&self) -> bool {
        self.element_text_ranges
            .as_ref()
            .is_some_and(|ranges| ranges.tracks_raw_text())
    }

    #[cfg(test)]
    pub(crate) fn tracks_text_ranges(&self) -> bool {
        self.element_text_ranges
            .as_ref()
            .is_some_and(|ranges| ranges.tracks_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Query, Save};

    #[test]
    fn with_capacity_reserves_arenas_conservatively() {
        let store = Store::with_capacity(30_000);

        assert_eq!(store.elements.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 30_000);
        assert_eq!(store.text.text.capacity(), 30_000);
        assert!(!store.tracks_element_text_ranges());
    }

    #[test]
    fn with_capacity_options_can_skip_text_reservation() {
        let store = Store::with_capacity_options(
            30_000,
            CapacityOptions {
                reserve_raw_text: false,
                reserve_text: false,
                ..CapacityOptions::default()
            },
        );

        assert_eq!(store.elements.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 0);
        assert_eq!(store.text.text.capacity(), 0);
        assert!(!store.tracks_element_text_ranges());
    }

    #[test]
    fn with_capacity_options_can_reserve_text_buffers() {
        let store = Store::with_capacity_options(
            30_000,
            CapacityOptions {
                reserve_raw_text: true,
                reserve_text: true,
                ..CapacityOptions::default()
            },
        );

        assert_eq!(store.elements.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 30_000);
        assert_eq!(store.text.text.capacity(), 30_000);
        assert!(!store.tracks_element_text_ranges());
    }

    #[test]
    fn with_capacity_options_can_reserve_raw_text_only() {
        let store = Store::with_capacity_options(
            30_000,
            CapacityOptions {
                reserve_raw_text: true,
                reserve_text: false,
                ..CapacityOptions::default()
            },
        );

        assert_eq!(store.text.raw_text.capacity(), 30_000);
        assert_eq!(store.text.text.capacity(), 0);
        assert!(!store.tracks_element_text_ranges());
    }

    #[test]
    fn parse_inner_html_only_skips_text_range_sidecar() {
        let html = "<p>one</p><p>two</p>";
        let query = Query::all("p", Save::only_inner_html()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();
        assert!(!store.tracks_element_text_ranges());
        assert_eq!(store.elements.len(), 2);
    }

    #[test]
    fn parse_text_modes_allocate_independent_range_vectors() {
        let html = "<p class=\"x\">A</p><p>B</p>";
        let queries = [
            Query::all("p", Save::only_text()).unwrap().build(),
            Query::all("p.x", Save::only_raw_text()).unwrap().build(),
        ];
        let store = crate::parse(html, &queries).unwrap();
        assert!(store.tracks_element_text_ranges());
        assert!(store.tracks_raw_text_ranges());
        assert!(store.tracks_text_ranges());
        assert_eq!(store.elements.len(), 3);
    }

    #[test]
    fn query_aware_capacity_keeps_unmatched_text_storage_lazy() {
        let html = "<main><div data-a='1'>outside</div><p>more</p></main>";
        let query = Query::all(".missing", Save::only_text()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();

        assert_eq!(store.text.raw_text.capacity(), 0);
        assert_eq!(store.text.text.capacity(), 0);
        assert!(!store.tracks_element_text_ranges());
    }

    #[test]
    fn one_text_mode_does_not_allocate_ranges_for_the_other() {
        let html = "<p>A</p><p>B</p>";
        let query = Query::all("p", Save::only_text()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();

        assert!(!store.tracks_raw_text_ranges());
        assert!(store.tracks_text_ranges());
    }

    #[test]
    fn element_layout_drops_inactive_text_range_fields() {
        use std::mem::size_of;
        if size_of::<usize>() == 8 {
            // main Element was 136 with one Option<Range>; with text ranges moved
            // to the sidecar, Element should be smaller than that.
            assert!(
                size_of::<Element>() <= 128,
                "Element unexpectedly large: {}",
                size_of::<Element>()
            );
        }
    }

    #[test]
    fn with_capacity_options_uses_custom_element_and_attribute_ratios() {
        let store = Store::with_capacity_options(
            1200,
            CapacityOptions {
                element_bytes_per_slot: 12,
                attribute_bytes_per_slot: 6,
                ..CapacityOptions::default()
            },
        );

        assert_eq!(store.elements.capacity(), 100);
        assert_eq!(store.attributes.capacity(), 200);
    }

    #[test]
    fn with_capacity_options_handles_zero_ratios_without_panicking() {
        let store = Store::with_capacity_options(
            16,
            CapacityOptions {
                element_bytes_per_slot: 0,
                attribute_bytes_per_slot: 0,
                ..CapacityOptions::default()
            },
        );

        assert_eq!(store.elements.capacity(), 16);
        assert_eq!(store.attributes.capacity(), 16);
    }

    fn store_for<'q>(queries: &'q [Query<'q>]) -> Store<'q, 'q> {
        let mut store = Store::default();
        store.set_sections(&Program::compile(queries));
        store
    }

    fn element(name: &str) -> crate::XHtmlElement<'_> {
        crate::XHtmlElement {
            name,
            ..Default::default()
        }
    }

    #[test]
    fn results_are_grouped_by_parent_then_section_in_document_order() {
        let queries = [Query::all("main > section", Save::all())
            .unwrap()
            .then(|section| {
                Ok([
                    section.all("> a[href]", Save::all())?,
                    section.all("div a", Save::all())?,
                ])
            })
            .unwrap()
            .build()];
        let mut store = store_for(&queries);
        let spec = |index: usize| &queries[0].queries[index];

        let first = store.add_row(SectionIndex(0), spec(0), &element("section"));
        store.add_edge(None, first);
        let deep = store.add_row(SectionIndex(2), spec(2), &element("a"));
        store.add_edge(Some(first), deep);
        let direct = store.add_row(SectionIndex(1), spec(1), &element("a"));
        store.add_edge(Some(first), direct);
        let second = store.add_row(SectionIndex(0), spec(0), &element("section"));
        store.add_edge(None, second);
        store.finish();

        assert_eq!(store.results("main > section"), Some(&[first, second][..]));
        assert_eq!(store.child_results(first, "> a[href]"), Some(&[direct][..]));
        assert_eq!(store.child_results(first, "div a"), Some(&[deep][..]));
        assert_eq!(store.nested_results(first, 1), Some(&[deep][..]));
        assert_eq!(store.child_results(second, "div a"), None);
        assert_eq!(
            store.results("> a[href]"),
            None,
            "nested sections are not roots"
        );
    }

    #[test]
    fn one_row_can_be_listed_under_several_parents() {
        let queries = [Query::all("div", Save::none())
            .unwrap()
            .all("a", Save::none())
            .unwrap()
            .build()];
        let mut store = store_for(&queries);

        let outer = store.add_row(SectionIndex(0), &queries[0].queries[0], &element("div"));
        store.add_edge(None, outer);
        let inner = store.add_row(SectionIndex(0), &queries[0].queries[0], &element("div"));
        store.add_edge(None, inner);
        let link = store.add_row(SectionIndex(1), &queries[0].queries[1], &element("a"));
        store.add_edge(Some(outer), link);
        store.add_edge(Some(inner), link);
        store.finish();

        assert_eq!(store.elements.len(), 3);
        assert_eq!(store.child_results(outer, "a"), Some(&[link][..]));
        assert_eq!(store.child_results(inner, "a"), Some(&[link][..]));
    }

    #[test]
    fn queries_with_the_same_selector_keep_separate_results() {
        let queries = [
            Query::all("a", Save::none()).unwrap().build(),
            Query::all("a", Save::all()).unwrap().build(),
        ];
        let store = crate::parse("<a href=x>1</a><a>2</a>", &queries).unwrap();

        assert_eq!(store.query(0).unwrap().count(), 2);
        assert_eq!(store.query(1).unwrap().count(), 2);
        assert!(store.query(0).unwrap().all(|a| a.inner_html.is_none()));
        assert!(store.query(1).unwrap().all(|a| a.inner_html.is_some()));
        // `get` resolves a shared selector to the first query.
        assert!(store.get("a").unwrap().all(|a| a.inner_html.is_none()));
        assert!(store.query(2).is_none());
    }

    #[test]
    fn test_multi_root_queries() {
        let queries = &[
            Query::all("span", Save::all()).unwrap().build(),
            Query::all("a", Save::all()).unwrap().build(),
        ];
        let mut store = store_for(queries);

        let span = store.add_row(SectionIndex(0), &queries[0].queries[0], &element("span"));
        store.add_edge(None, span);
        let a = store.add_row(SectionIndex(1), &queries[1].queries[0], &element("a"));
        store.add_edge(None, a);
        store.finish();

        assert_eq!(store.get("span").unwrap().count(), 1);
        assert_eq!(store.get("a").unwrap().count(), 1);
        assert_eq!(store.query(1).unwrap().next().unwrap().name, "a");
    }
}
