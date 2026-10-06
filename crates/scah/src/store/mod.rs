use crate::Attribute;
use crate::QuerySection;
use scah_query_ir::{Program, SectionIndex};
use std::ops::Range;

mod text;
pub(crate) use text::{TextStore, TextTape, trim_collapsed_range};
mod arena;
mod attributes;
mod columns;
mod element;

pub use arena::{
    Arena,
    id::{AttributeId, ElementId},
};
use columns::{AttributeCells, ColumnPlan, Columns, RowHead, Span};

pub use element::{ElementRef, Elements};

/// A query section as the store sees it: enough to resolve lookups.
#[derive(Debug, Clone, PartialEq)]
struct StoreSection<'query> {
    selector: &'query str,
    /// Position among its parent's nested sections, or among the root
    /// sections: the result slot it fills in each parent group.
    slot: u32,
    /// This section's nested sections, as a range of `Store::children`.
    children: (u32, u32),
}

/// The result set returned by [`parse`](crate::parse).
///
/// A `Store` holds flat tables: one row per element and query section that
/// saved it, stored as one column per field (name, attributes, inner HTML,
/// text), and an index from each parent (the document, or a row of the
/// parent section) to its results, grouped by section in document order.
/// Rows are read through [`ElementRef`] handles. Look results up by
/// selector string with [`Store::get`], or by position with
/// [`Store::query`].
///
/// Strings are stored as 32-bit `(offset, len)` spans, so the parsed HTML
/// must be shorter than 4 GiB.
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
/// assert_eq!(anchors[0].attribute("href"), Some("x"));
/// ```
#[derive(Debug, PartialEq)]
pub struct Store<'html, 'query> {
    /// The parsed HTML, which name, class, id and inner HTML spans index.
    html: &'html str,
    /// One row per element and section that saved it. An element saved by a
    /// nested section under several parent matches has one row, listed
    /// under each parent.
    columns: Columns,
    /// Attributes other than `class` and `id` of rows that saved them.
    pub(crate) attributes: Arena<Attribute<'html>, AttributeId>,
    /// Accumulated raw-text and normalized-text buffers shared by all elements.
    pub(crate) text: TextStore,
    /// A text range did not fit a span.
    overflowed: bool,
    /// Sections of the parsed queries, in declaration order.
    sections: Box<[StoreSection<'query>]>,
    /// Root sections, one per query, in query order.
    roots: Box<[u32]>,
    /// Nested sections of every section, in declaration order.
    children: Box<[u32]>,
    /// `(parent group, row)` per save, in document order. Group 0 is the
    /// document; group `r + 1` is row `r`. Not recorded when every section
    /// is a root: then every row is a document result.
    edges: Vec<(u32, u32)>,
    record_edges: bool,
    /// Group `g` owns one result slot per section it can hold results of
    /// (the roots for the document, the row section's nested sections
    /// otherwise), starting at slot `slot_starts[g]`. Built by
    /// [`Store::finish`].
    slot_starts: Vec<u32>,
    /// Results of slot `s` are `results[slot_offsets[s]..slot_offsets[s + 1]]`,
    /// in document order.
    slot_offsets: Vec<u32>,
    results: Vec<ElementId>,
    #[cfg(any(debug_assertions, test))]
    pub trace: crate::debug::TraceStore<'html, 'query>,
}

/// Heap bytes a [`Store`] holds, by role, as reported by
/// [`Store::heap_usage`]. Each figure is allocated capacity, not length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapUsage {
    /// Row columns and each row's section.
    pub rows: usize,
    /// The result index.
    pub results: usize,
    /// The attribute tape.
    pub attributes: usize,
    /// The raw-text and normalized-text buffers.
    pub text: usize,
}

impl HeapUsage {
    /// Sum of all roles.
    pub fn total(&self) -> usize {
        self.rows + self.results + self.attributes + self.text
    }
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

/// Whether a looked-up selector names a section's selector. Callers usually
/// pass the same string the query was built from, so compare addresses
/// before bytes.
#[inline]
fn same_selector(section: &str, selector: &str) -> bool {
    section.len() == selector.len()
        && (std::ptr::eq(section.as_ptr(), selector.as_ptr()) || section == selector)
}

impl<'html, 'query: 'html> Default for Store<'html, 'query> {
    fn default() -> Self {
        Self {
            html: "",
            columns: Columns::default(),
            text: TextStore::new(),
            attributes: Arena::new(),
            overflowed: false,
            sections: Box::default(),
            roots: Box::default(),
            children: Box::default(),
            edges: Vec::new(),
            record_edges: true,
            slot_starts: Vec::new(),
            slot_offsets: Vec::new(),
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
    /// | Storage      | Reservation                        |
    /// |------------- |----------------------------------- |
    /// | element rows | `capacity / element_bytes_per_slot` |
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

        let mut columns = Columns::default();
        columns.rows.reserve(element_slots);
        Self {
            html: "",
            columns,
            overflowed: false,
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
            sections: Box::default(),
            roots: Box::default(),
            children: Box::default(),
            edges: Vec::new(),
            record_edges: true,
            slot_starts: Vec::new(),
            slot_offsets: Vec::new(),
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
    ///     println!("{}", li.text().unwrap_or_default());
    /// }
    /// ```
    #[inline]
    pub fn get(&self, selector: &str) -> Option<Elements<'_, 'html, 'query>> {
        Some(Elements::new(self, self.results(selector)?))
    }

    /// Results saved for the query at `index` in the slice given to
    /// [`parse`](crate::parse).
    ///
    /// Returns `None` when there is no such query or it matched nothing.
    #[inline]
    pub fn query(&self, index: usize) -> Option<Elements<'_, 'html, 'query>> {
        Some(Elements::new(self, self.query_results(index)?))
    }

    /// The row with id `id`.
    #[inline]
    pub fn element(&self, id: ElementId) -> Option<ElementRef<'_, 'html, 'query>> {
        (id.index() < self.len()).then(|| ElementRef::new(self, id))
    }

    /// Every row, in the order the rows were saved, regardless of query.
    pub fn elements(&self) -> impl ExactSizeIterator<Item = ElementRef<'_, 'html, 'query>> {
        (0..self.len()).map(|row| ElementRef::new(self, ElementId::from(row)))
    }

    /// Number of rows: elements saved, counting an element once per
    /// section that saved it.
    #[inline]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Number of attributes saved, other than `class` and `id`.
    pub fn attribute_count(&self) -> usize {
        self.attributes.len()
    }

    /// Heap bytes held by this store, by role.
    pub fn heap_usage(&self) -> HeapUsage {
        fn bytes<T>(vec: &Vec<T>) -> usize {
            vec.capacity() * std::mem::size_of::<T>()
        }
        let columns = &self.columns;
        HeapUsage {
            rows: bytes(&columns.rows)
                + bytes(&columns.attributes)
                + bytes(&columns.inner_html)
                + bytes(&columns.raw_text)
                + bytes(&columns.text),
            results: bytes(&self.edges)
                + bytes(&self.slot_starts)
                + bytes(&self.slot_offsets)
                + bytes(&self.results),
            attributes: bytes(&*self.attributes),
            text: self.text.raw_text.capacity() + self.text.text.capacity(),
        }
    }

    /// Rows reserved in the name column.
    #[cfg(test)]
    pub(crate) fn row_capacity(&self) -> usize {
        self.columns.rows.capacity()
    }

    /// Whether nothing was saved.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Row ids of [`Store::get`]'s results.
    #[inline]
    pub fn results(&self, selector: &str) -> Option<&[ElementId]> {
        let root = self.roots.iter().position(|&section| {
            same_selector(self.sections[section as usize].selector, selector)
        })?;
        self.slot_results(root)
    }

    /// Row ids of [`Store::query`]'s results.
    #[inline]
    pub fn query_results(&self, index: usize) -> Option<&[ElementId]> {
        if index >= self.roots.len() {
            return None;
        }
        self.slot_results(index)
    }

    /// Row ids saved under `parent` by its first nested section whose
    /// selector is `selector`.
    #[inline]
    pub fn child_results(&self, parent: ElementId, selector: &str) -> Option<&[ElementId]> {
        let child = self.child_sections(parent).iter().position(|&section| {
            same_selector(self.sections[section as usize].selector, selector)
        })?;
        self.nested_results(parent, child)
    }

    /// Row ids saved under `parent` by its nested section at `index`, in
    /// the order the sections were declared.
    #[inline]
    pub fn nested_results(&self, parent: ElementId, index: usize) -> Option<&[ElementId]> {
        if index >= self.child_sections(parent).len() {
            return None;
        }
        let start = *self.slot_starts.get(parent.index() + 1)? as usize;
        self.slot_results(start + index)
    }

    /// Number of results across all parents; a row listed under several
    /// parents counts once per parent.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Selector of the query at `index`.
    pub fn query_selector(&self, index: usize) -> Option<&'query str> {
        let section = *self.roots.get(index)?;
        Some(self.sections[section as usize].selector)
    }

    /// Selector of `parent`'s nested section at `index`.
    pub fn nested_selector(&self, parent: ElementId, index: usize) -> Option<&'query str> {
        let section = *self.child_sections(parent).get(index)?;
        Some(self.sections[section as usize].selector)
    }

    /// The HTML a span of the name, class, id or inner HTML column covers.
    #[inline(always)]
    fn html_value(&self, span: Span) -> Option<&'html str> {
        let range = span.range()?;
        debug_assert!(self.html.get(range.clone()).is_some());
        // SAFETY: these spans are only built by `html_span`, from `&str`
        // subslices of `html`, so the range is in bounds and both ends are
        // on character boundaries.
        Some(unsafe { self.html.get_unchecked(range) })
    }

    /// Span of `value`, a string borrowed from the parsed HTML.
    ///
    /// Every string the parser hands over is a subslice of the input. An
    /// empty string from elsewhere is stored as an empty span.
    #[inline(always)]
    fn html_span(&self, value: &str) -> Span {
        let offset = value
            .as_ptr()
            .addr()
            .wrapping_sub(self.html.as_ptr().addr());
        if offset <= self.html.len() && value.len() <= self.html.len() - offset {
            // The HTML is shorter than `u32::MAX` bytes (`set_html`).
            Span::new(offset, value.len())
        } else {
            Self::foreign_span(value)
        }
    }

    #[cold]
    #[inline(never)]
    fn foreign_span(value: &str) -> Span {
        assert!(
            value.is_empty(),
            "stored strings must borrow from the parsed HTML"
        );
        Span::new(0, 0)
    }

    #[inline(always)]
    fn optional_html_span(&self, value: Option<&str>) -> Span {
        value.map_or(Span::ABSENT, |value| self.html_span(value))
    }

    /// Record the HTML being parsed, which stored strings borrow from.
    ///
    /// # Panics
    ///
    /// If `html` is not shorter than `u32::MAX` bytes, or rows were already
    /// saved from a different document.
    pub(crate) fn set_html(&mut self, html: &'html str) {
        assert!(
            html.len() < u32::MAX as usize,
            "the store indexes HTML with 32-bit offsets"
        );
        assert!(
            self.is_empty() || self.bound_to(html.as_bytes()),
            "a store's results borrow from one document"
        );
        self.html = html;
    }

    /// Whether the store reads strings from `source`.
    #[inline]
    pub(crate) fn bound_to(&self, source: &[u8]) -> bool {
        std::ptr::eq(self.html.as_bytes(), source)
    }

    /// Whether a text range did not fit in 32 bits, so some text is missing.
    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Nested sections of the section that saved `row`.
    #[inline]
    fn child_sections(&self, row: ElementId) -> &[u32] {
        let (start, end) = self.sections[self.columns.rows[row.index()].section as usize].children;
        &self.children[start as usize..end as usize]
    }

    /// Results of `slot`, or `None` when empty.
    #[inline]
    fn slot_results(&self, slot: usize) -> Option<&[ElementId]> {
        let start = *self.slot_offsets.get(slot)? as usize;
        let end = *self.slot_offsets.get(slot + 1)? as usize;
        (start < end).then(|| &self.results[start..end])
    }

    /// Record the sections of the queries being parsed.
    pub(crate) fn set_sections(&mut self, program: &Program<'query>) {
        let count = program.section_count();
        let parent = |index: usize| program.section_parent(SectionIndex(index as u32));

        // Each section's nested sections form one run of `children`, in
        // declaration order: count them, then place them.
        let mut starts = vec![0_u32; count + 1];
        for index in 0..count {
            if let Some(parent) = parent(index) {
                starts[parent.index() + 1] += 1;
            }
        }
        Self::prefix_sum(&mut starts);
        let mut next = starts.clone();
        let mut children = vec![0_u32; starts[count] as usize];
        let mut roots = Vec::new();
        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let slot = match parent(index) {
                None => {
                    roots.push(index as u32);
                    roots.len() - 1
                }
                Some(parent) => {
                    let position = &mut next[parent.index()];
                    children[*position as usize] = index as u32;
                    *position += 1;
                    (*position - 1 - starts[parent.index()]) as usize
                }
            };
            sections.push(StoreSection {
                selector: program.section(SectionIndex(index as u32)).source,
                slot: slot as u32,
                children: (starts[index], starts[index + 1]),
            });
        }
        self.sections = sections.into();
        self.roots = roots.into();
        self.children = children.into();
        self.record_edges = program.features().has_scoped_sections;

        let mut plan = ColumnPlan::default();
        for index in 0..count {
            plan.add(program.section(SectionIndex(index as u32)).save);
        }
        self.columns.plan = plan;
        // Give the optional columns the rows reserved so far.
        self.columns.reserve_optional();
    }

    /// Save `element` as a new row of `section`.
    ///
    /// The row is not a result until [`Store::add_edge`] lists it under a
    /// parent.
    #[inline]
    pub(crate) fn add_row(
        &mut self,
        section: SectionIndex,
        spec: &QuerySection<'query>,
        element: &crate::XHtmlElement<'html>,
    ) -> ElementId {
        let row = ElementId::from(self.len());
        let attributes = if spec.save.attributes {
            AttributeCells {
                class: self.optional_html_span(element.class),
                id: self.optional_html_span(element.id),
                others: self
                    .attributes
                    .attribute_slice_to_range(element.attributes)
                    .map_or(Span::ABSENT, |range| {
                        // Each attribute spans at least one byte of the HTML.
                        Span::new(range.start as usize, (range.end - range.start) as usize)
                    }),
            }
        } else {
            AttributeCells::ABSENT
        };
        let head = RowHead {
            name: self.html_span(element.name),
            section: section.0,
        };
        self.columns.push(head, attributes);
        row
    }

    /// List `row` among the results of `parent` (the document when `None`).
    #[inline]
    pub(crate) fn add_edge(&mut self, parent: Option<ElementId>, row: ElementId) {
        if self.record_edges {
            let group = parent.map_or(0, |parent| parent.0 + 1);
            self.edges.push((group, row.0));
        } else {
            debug_assert!(
                parent.is_none(),
                "root-only stores list rows under the document"
            );
        }
    }

    /// Build the result index: assign every group its slots, then place
    /// each saved edge in its slot, keeping document order (one stable
    /// counting sort).
    pub(crate) fn finish(&mut self) {
        if !self.slot_offsets.is_empty() {
            return;
        }
        self.slot_starts.reserve(self.len() + 2);
        self.slot_starts.extend([0, self.roots.len() as u32]);
        if self.record_edges {
            let mut next = self.roots.len() as u32;
            for row in &self.columns.rows {
                let (start, end) = self.sections[row.section as usize].children;
                next += end - start;
                self.slot_starts.push(next);
            }
        }
        let slots = *self.slot_starts.last().unwrap_or(&0) as usize;
        self.slot_offsets.resize(slots + 1, 0);

        if !self.record_edges {
            // Every row is a document result in its section's root slot.
            let rows = self.len() as u32;
            if self.roots.len() == 1 {
                self.slot_offsets[1] = rows;
                self.results.extend((0..rows).map(ElementId));
                return;
            }
            let slot_of =
                |row: u32| self.sections[self.columns.rows[row as usize].section as usize].slot;
            for row in 0..rows {
                self.slot_offsets[slot_of(row) as usize + 1] += 1;
            }
            Self::prefix_sum(&mut self.slot_offsets);
            if (1..rows).all(|row| slot_of(row - 1) <= slot_of(row)) {
                self.results.extend((0..rows).map(ElementId));
                return;
            }
            let mut next = self.slot_offsets.clone();
            self.results.resize(rows as usize, ElementId(0));
            for row in 0..rows {
                let slot = &mut next[slot_of(row) as usize];
                self.results[*slot as usize] = ElementId(row);
                *slot += 1;
            }
            return;
        }

        // Replace each edge's group by its slot, counting slot sizes.
        let mut edges = std::mem::take(&mut self.edges);
        let mut sorted = true;
        let mut previous = 0;
        for edge in &mut edges {
            let section = self.columns.rows[edge.1 as usize].section as usize;
            let slot = self.slot_starts[edge.0 as usize] + self.sections[section].slot;
            sorted &= previous <= slot;
            previous = slot;
            edge.0 = slot;
            self.slot_offsets[slot as usize + 1] += 1;
        }
        Self::prefix_sum(&mut self.slot_offsets);
        if sorted {
            self.results
                .extend(edges.iter().map(|&(_, row)| ElementId(row)));
            return;
        }
        let mut next = self.slot_offsets.clone();
        self.results.resize(edges.len(), ElementId(0));
        for (slot, row) in edges {
            let slot = &mut next[slot as usize];
            self.results[*slot as usize] = ElementId(row);
            *slot += 1;
        }
    }

    fn prefix_sum(counts: &mut [u32]) {
        for index in 1..counts.len() {
            counts[index] += counts[index - 1];
        }
    }

    /// Record the content of `element_id`, once its element closed.
    pub(crate) fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    ) {
        let row = element_id.index();
        assert!(row < self.len());

        #[cfg(any(debug_assertions, test))]
        let tag = self
            .html_value(self.columns.rows[row].name)
            .unwrap_or_default();
        #[cfg(any(debug_assertions, test))]
        let has_inner_html = inner_html.is_some();
        #[cfg(any(debug_assertions, test))]
        let has_raw_text = raw_text.is_some();
        #[cfg(any(debug_assertions, test))]
        let has_text = text.is_some();

        if inner_html.is_some() {
            let span = self.optional_html_span(inner_html);
            self.columns.inner_html[row] = span;
        }
        for (range, column) in [
            (raw_text, &mut self.columns.raw_text),
            (text, &mut self.columns.text),
        ] {
            let Some(range) = range else { continue };
            match Span::of(range) {
                Some(span) => column[row] = span,
                None => self.overflowed = true,
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Query, Save};

    /// Source of the element names the table tests store.
    static HTML: &str = "section a div span";

    fn tracks_text(store: &Store) -> bool {
        !store.columns.text.is_empty()
    }

    fn tracks_raw_text(store: &Store) -> bool {
        !store.columns.raw_text.is_empty()
    }

    #[test]
    fn with_capacity_reserves_arenas_conservatively() {
        let store = Store::with_capacity(30_000);

        assert_eq!(store.columns.rows.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 30_000);
        assert_eq!(store.text.text.capacity(), 30_000);
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
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

        assert_eq!(store.columns.rows.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 0);
        assert_eq!(store.text.text.capacity(), 0);
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
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

        assert_eq!(store.columns.rows.capacity(), 30_000 / 48);
        assert_eq!(store.attributes.capacity(), 30_000 / 24);
        assert_eq!(store.text.raw_text.capacity(), 30_000);
        assert_eq!(store.text.text.capacity(), 30_000);
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
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
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
    }

    #[test]
    fn parse_inner_html_only_skips_text_range_sidecar() {
        let html = "<p>one</p><p>two</p>";
        let query = Query::all("p", Save::only_inner_html()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn parse_text_modes_allocate_independent_range_vectors() {
        let html = "<p class=\"x\">A</p><p>B</p>";
        let queries = [
            Query::all("p", Save::only_text()).unwrap().build(),
            Query::all("p.x", Save::only_raw_text()).unwrap().build(),
        ];
        let store = crate::parse(html, &queries).unwrap();
        assert!(tracks_text(&store) || tracks_raw_text(&store));
        assert!(tracks_raw_text(&store));
        assert!(tracks_text(&store));
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn query_aware_capacity_keeps_unmatched_text_storage_lazy() {
        let html = "<main><div data-a='1'>outside</div><p>more</p></main>";
        let query = Query::all(".missing", Save::only_text()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();

        assert_eq!(store.text.raw_text.capacity(), 0);
        assert_eq!(store.text.text.capacity(), 0);
        assert!(!tracks_text(&store) && !tracks_raw_text(&store));
    }

    #[test]
    fn one_text_mode_does_not_allocate_ranges_for_the_other() {
        let html = "<p>A</p><p>B</p>";
        let query = Query::all("p", Save::only_text()).unwrap().build();
        let queries = [query];
        let store = crate::parse(html, &queries).unwrap();

        assert!(!tracks_raw_text(&store));
        assert!(tracks_text(&store));
    }

    #[test]
    fn row_cells_are_compact() {
        assert_eq!(std::mem::size_of::<Span>(), 8);
        assert_eq!(std::mem::size_of::<AttributeCells>(), 24);
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

        assert_eq!(store.columns.rows.capacity(), 100);
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

        assert_eq!(store.columns.rows.capacity(), 16);
        assert_eq!(store.attributes.capacity(), 16);
    }

    fn store_for<'q>(queries: &'q [Query<'q>]) -> Store<'q, 'q> {
        let mut store = Store::default();
        store.set_html(HTML);
        store.set_sections(&Program::compile(queries));
        store
    }

    fn element(name: &str) -> crate::XHtmlElement<'static> {
        let start = HTML.find(name).expect("name in HTML");
        crate::XHtmlElement {
            name: &HTML[start..start + name.len()],
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
    fn positional_lookups_match_selector_lookups() {
        let queries = [
            Query::all("ul", Save::none())
                .unwrap()
                .then(|list| {
                    Ok([
                        list.all("> li", Save::none())?,
                        list.first("a", Save::none())?,
                    ])
                })
                .unwrap()
                .build(),
            Query::all("p", Save::none()).unwrap().build(),
        ];
        let store = crate::parse(
            "<ul><li><a>1</a></li><li>2</li></ul><p></p><ul></ul>",
            &queries,
        )
        .unwrap();

        let lists: Vec<_> = store.query_results(0).unwrap().to_vec();
        assert_eq!(lists.len(), 2);
        assert_eq!(store.query_selector(0), Some("ul"));
        assert_eq!(store.query_selector(1), Some("p"));
        assert_eq!(store.query_selector(2), None);
        assert_eq!(store.query_results(1), store.results("p"));
        assert_eq!(store.query_results(2), None);

        let [full, empty] = [lists[0], lists[1]];
        assert_eq!(store.nested_results(full, 0).map(<[_]>::len), Some(2));
        assert_eq!(
            store.nested_results(full, 0),
            store.child_results(full, "> li")
        );
        assert_eq!(
            store.nested_results(full, 1),
            store.child_results(full, "a")
        );
        assert_eq!(store.nested_selector(full, 1), Some("a"));
        assert_eq!(store.nested_results(full, 2), None);
        assert_eq!(store.nested_selector(full, 2), None);
        assert_eq!(store.nested_results(empty, 0), None);

        // Leaf rows have no nested sections.
        let item = store.nested_results(full, 0).unwrap()[0];
        assert_eq!(store.nested_results(item, 0), None);
        assert_eq!(store.child_results(item, "a"), None);
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

        assert_eq!(store.len(), 3);
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
        assert!(store.query(0).unwrap().all(|a| a.inner_html().is_none()));
        assert!(store.query(1).unwrap().all(|a| a.inner_html().is_some()));
        // `get` resolves a shared selector to the first query.
        assert!(store.get("a").unwrap().all(|a| a.inner_html().is_none()));
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
        assert_eq!(store.query(1).unwrap().next().unwrap().name(), "a");
    }
}
