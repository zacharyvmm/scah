//! Struct-of-arrays result storage.
//!
//! [`ColumnarStore`] records the same information as [`Store`](super::Store)
//! but keeps one column per element field instead of one struct per element.
//! Strings are `(u32 offset, u32 len)` spans into the parsed HTML, links are
//! `u32` element or query-node ids, and optional fields only get a column
//! when some query saves them.

use std::fmt;
use std::ops::{Deref, Range};

use super::{Arena, AttributeId, ElementId, Nullable, ResultSink, TextStore};
use crate::{Attribute, QuerySection, QuerySpec, XHtmlElement};

/// Sentinel for an absent element, query node, or span.
const NONE: u32 = u32::MAX;

/// Largest number of elements a [`ColumnarStore`] can hold. Element ids are
/// `u32` and [`NONE`] is reserved.
const MAX_ELEMENTS: usize = NONE as usize;

/// Approximate HTML bytes per reserved element slot, as in
/// [`CapacityOptions`](crate::CapacityOptions).
const ELEMENT_BYTES_PER_SLOT: usize = 48;

/// Approximate HTML bytes per reserved attribute slot, as in
/// [`CapacityOptions`](crate::CapacityOptions).
const ATTRIBUTE_BYTES_PER_SLOT: usize = 24;

/// A `(offset, len)` byte span into the HTML, the attribute tape, or a text
/// tape. An `offset` of [`NONE`] marks an absent value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    offset: u32,
    len: u32,
}

impl Span {
    const ABSENT: Self = Self {
        offset: NONE,
        len: 0,
    };

    #[inline(always)]
    fn is_present(self) -> bool {
        self.offset != NONE
    }

    #[inline(always)]
    fn range(self) -> Range<usize> {
        let start = self.offset as usize;
        start..start + self.len as usize
    }
}

/// Which optional element columns a parse fills.
///
/// Decided once from the queries' [`Save`](crate::Save) flags. A column that
/// is off stays an empty `Vec`; it is switched on (and back-filled with
/// absent values) only if a write arrives anyway, so the plan affects memory
/// and speed but never results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ColumnPlan {
    attributes: bool,
    inner_html: bool,
    raw_text: bool,
    text: bool,
    nested: bool,
}

impl ColumnPlan {
    fn from_queries<'query, Q: QuerySpec<'query>>(queries: &[Q]) -> Self {
        let mut plan = Self::default();
        for query in queries {
            let sections = query.queries();
            plan.nested |= sections.len() > 1;
            for section in sections {
                plan.attributes |= section.save.attributes;
                plan.inner_html |= section.save.inner_html;
                plan.raw_text |= section.save.raw_text;
                plan.text |= section.save.text;
            }
        }
        plan
    }
}

/// Heap bytes held by a result store, split by role.
///
/// Reported by [`ColumnarStore::heap_usage`] and
/// [`Store::heap_usage`](super::Store::heap_usage) so the two layouts can be
/// compared. Every figure is `capacity * size_of::<T>()` for the vectors in
/// that group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapUsage {
    /// Per-element storage: element structs or element columns, plus the
    /// per-element text-range storage.
    pub elements: usize,
    /// Query-result list headers and interned selectors.
    pub query_nodes: usize,
    /// The shared attribute tape.
    pub attributes: usize,
    /// The shared raw-text and normalized-text tapes.
    pub text_tapes: usize,
}

impl HeapUsage {
    /// Sum of all groups.
    pub fn total(&self) -> usize {
        self.elements + self.query_nodes + self.attributes + self.text_tapes
    }
}

#[inline(always)]
fn vec_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

/// One query-result list: the matches of one selector under one parent
/// (or at the root), linked through `next_sibling`.
///
/// Unlike elements, query nodes stay row-oriented: a lookup always reads
/// the selector and `next` together, then `first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueryNodeRow {
    /// Index into the interned selector table.
    selector: u32,
    /// Next result list under the same parent, or [`NONE`].
    next: u32,
    /// First element id; once lists are compacted, the start of the list's
    /// run in `list_ids`.
    first: u32,
    /// Last element id; once lists are compacted, the end (exclusive) of the
    /// list's run in `list_ids`.
    last: u32,
}

/// Matches stored column by column.
///
/// Returned by [`parse_columnar`](crate::parse_columnar). It answers the same
/// questions as [`Store`](crate::Store) with the same results, but elements
/// are lightweight [`ElementRef`] handles instead of structs:
///
/// ```rust
/// use scah::{Query, Save, parse_columnar};
///
/// let html = "<div><a href='x'>Link1</a><a href='y'>Link2</a></div>";
/// let queries = &[Query::all("a", Save::all()).unwrap().build()];
/// let store = parse_columnar(html, queries).unwrap();
///
/// let hrefs: Vec<_> = store
///     .get("a")
///     .unwrap()
///     .map(|a| a.attribute("href"))
///     .collect();
/// assert_eq!(hrefs, [Some("x"), Some("y")]);
/// ```
///
/// # Layout
///
/// | Column | Element | Present when |
/// |--------|---------|--------------|
/// | `names` | 8-byte span | always |
/// | `classes`, `ids`, `attributes` | 8-byte span each | a query saves attributes |
/// | `inner_html` | 8-byte span | a query saves inner HTML |
/// | `raw_text`, `text` | 8-byte span each | a query saves that text mode |
/// | `first_child_query` | `u32` | a query has nested sections |
/// | `next_sibling` | `u32` | while parsing |
/// | `list_ids` | `u32` | after parsing |
///
/// Result lists are linked through `next_sibling` while parsing, because
/// lists interleave in document order. When the parse ends they are copied
/// into `list_ids` as one contiguous run per list and the link column is
/// freed, so iterating a result list scans a slice of ids.
/// So an element costs 12 bytes with no saved content and at most 60 bytes
/// with everything saved, against a fixed 112-byte [`Element`](crate::Element)
/// plus a 24-byte text-range slot per enabled text mode in [`Store`](crate::Store).
///
/// # Limits
///
/// Offsets and ids are `u32`, so the HTML must be shorter than `u32::MAX`
/// bytes and a parse may record fewer than `u32::MAX` elements.
/// [`parse_columnar`](crate::parse_columnar) reports either case as
/// [`ColumnarParseError::InputTooLarge`](crate::ColumnarParseError::InputTooLarge).
pub struct ColumnarStore<'html, 'query> {
    html: &'html str,
    plan: ColumnPlan,
    overflowed: bool,

    names: Vec<Span>,
    classes: Vec<Span>,
    ids: Vec<Span>,
    attributes: Vec<Span>,
    inner_html: Vec<Span>,
    raw_text: Vec<Span>,
    text: Vec<Span>,
    first_child_query: Vec<u32>,
    /// Result-list links while parsing; freed when lists are compacted.
    next_sibling: Vec<u32>,
    /// Every result list as a contiguous run of element ids, once compacted.
    list_ids: Vec<u32>,
    contiguous: bool,

    /// Distinct selector strings; query nodes refer to them by index.
    selectors: Vec<&'query str>,
    nodes: Vec<QueryNodeRow>,

    attribute_tape: Arena<Attribute<'html>, AttributeId>,
    text_tapes: TextStore,
}

impl<'html, 'query: 'html> ColumnarStore<'html, 'query> {
    /// Create an empty store for `html`, sized for a full parse when
    /// `reserve` is set. `html` must be shorter than `u32::MAX` bytes.
    pub(crate) fn for_queries<Q: QuerySpec<'query>>(
        html: &'html str,
        queries: &[Q],
        reserve: bool,
    ) -> Self {
        debug_assert!(html.len() < NONE as usize);
        let plan = ColumnPlan::from_queries(queries);
        let slots = if reserve {
            html.len() / ELEMENT_BYTES_PER_SLOT
        } else {
            0
        };
        let column = |enabled: bool| {
            if enabled {
                Vec::with_capacity(slots)
            } else {
                Vec::new()
            }
        };
        Self {
            html,
            plan,
            overflowed: false,
            names: Vec::with_capacity(slots),
            classes: column(plan.attributes),
            ids: column(plan.attributes),
            attributes: column(plan.attributes),
            inner_html: column(plan.inner_html),
            raw_text: column(plan.raw_text),
            text: column(plan.text),
            first_child_query: if plan.nested {
                Vec::with_capacity(slots)
            } else {
                Vec::new()
            },
            next_sibling: Vec::with_capacity(slots),
            list_ids: Vec::new(),
            contiguous: false,
            selectors: Vec::new(),
            nodes: Vec::new(),
            attribute_tape: if reserve && plan.attributes {
                Arena::with_capacity(html.len() / ATTRIBUTE_BYTES_PER_SLOT)
            } else {
                Arena::new()
            },
            text_tapes: TextStore::new(),
        }
    }

    /// Whether a value or element count exceeded the `u32` limits.
    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Number of matched elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no element matched.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The parsed HTML every element string borrows from.
    #[inline]
    pub fn html(&self) -> &'html str {
        self.html
    }

    /// Elements that matched the root query with this exact selector string,
    /// in document order.
    ///
    /// Returns `None` when the selector was not part of the parsed queries or
    /// matched nothing, exactly like [`Store::get`](crate::Store::get).
    pub fn get(&self, selector: &str) -> Option<ColumnarElements<'_, 'html, 'query>> {
        if self.nodes.is_empty() {
            return None;
        }
        self.find_node(0, selector).map(|node| self.list(node))
    }

    /// Store every result list as one contiguous run of element ids.
    ///
    /// While parsing, a list grows one element at a time in between other
    /// lists, so lists are linked through a per-element `next_sibling`
    /// column. Once parsing ends this pass copies each list into a single id
    /// array, in list order, and frees the link column, so
    /// [`ColumnarStore::get`] and [`ElementRef::get`] iterate a slice
    /// instead of following links. Ids and results are unchanged.
    pub(crate) fn compact_result_lists(&mut self) {
        debug_assert!(!self.contiguous, "result lists compacted twice");
        // Every element belongs to exactly one list.
        let mut list_ids = Vec::with_capacity(self.len());
        for row in &mut self.nodes {
            let start = list_ids.len() as u32;
            let mut cursor = row.first;
            while cursor != NONE {
                list_ids.push(cursor);
                cursor = self.next_sibling[cursor as usize];
            }
            row.first = start;
            row.last = list_ids.len() as u32;
        }
        debug_assert_eq!(list_ids.len(), self.len());
        self.list_ids = list_ids;
        self.next_sibling = Vec::new();
        self.contiguous = true;
    }

    /// Iterator over the elements of query node `node`.
    #[inline]
    fn list(&self, node: u32) -> ColumnarElements<'_, 'html, 'query> {
        debug_assert!(self.contiguous, "result lists read before compaction");
        let row = self.nodes[node as usize];
        // Once compacted, `first..last` is the list's run in `list_ids`.
        ColumnarElements {
            store: self,
            ids: self.list_ids[row.first as usize..row.last as usize].iter(),
        }
    }

    /// The element with this id, if any. Ids run from `0` to `len() - 1` in
    /// the order elements matched.
    #[inline]
    pub fn element(&self, id: u32) -> Option<ElementRef<'_, 'html, 'query>> {
        ((id as usize) < self.len()).then_some(ElementRef { store: self, id })
    }

    /// Every matched element in id order, regardless of query.
    pub fn elements(&self) -> impl ExactSizeIterator<Item = ElementRef<'_, 'html, 'query>> {
        (0..self.len() as u32).map(|id| ElementRef { store: self, id })
    }

    /// The shared attribute tape every [`ElementRef::attributes`] slice
    /// borrows from.
    pub fn attribute_buffer(&self) -> &[Attribute<'html>] {
        &self.attribute_tape
    }

    /// The shared buffer every [`ElementRef::raw_text`] slice borrows from.
    pub fn raw_text_buffer(&self) -> &[u8] {
        self.text_tapes.raw_text.as_bytes()
    }

    /// The shared buffer every [`ElementRef::text`] slice borrows from.
    pub fn text_buffer(&self) -> &[u8] {
        self.text_tapes.text.as_bytes()
    }

    /// Heap bytes held by this store, by role.
    pub fn heap_usage(&self) -> HeapUsage {
        HeapUsage {
            elements: vec_bytes(&self.names)
                + vec_bytes(&self.classes)
                + vec_bytes(&self.ids)
                + vec_bytes(&self.attributes)
                + vec_bytes(&self.inner_html)
                + vec_bytes(&self.raw_text)
                + vec_bytes(&self.text)
                + vec_bytes(&self.first_child_query)
                + vec_bytes(&self.next_sibling)
                + vec_bytes(&self.list_ids),
            query_nodes: vec_bytes(&self.selectors) + vec_bytes(&self.nodes),
            attributes: vec_bytes(&self.attribute_tape),
            text_tapes: self.text_tapes.raw_text.capacity() + self.text_tapes.text.capacity(),
        }
    }

    /// Walk the query-node chain from `head` for the node of `selector`.
    fn find_node(&self, head: u32, selector: &str) -> Option<u32> {
        let mut node = head;
        while node != NONE {
            let row = &self.nodes[node as usize];
            if self.selectors[row.selector as usize] == selector {
                return Some(node);
            }
            node = row.next;
        }
        None
    }

    /// Index of `source` in the selector table, adding it if new.
    #[inline]
    fn intern(&mut self, source: &'query str) -> u32 {
        for (index, known) in self.selectors.iter().enumerate() {
            if (known.as_ptr() == source.as_ptr() && known.len() == source.len())
                || *known == source
            {
                return index as u32;
            }
        }
        self.selectors.push(source);
        (self.selectors.len() - 1) as u32
    }

    /// Span of a string borrowed from the parsed HTML.
    ///
    /// Every string the parser hands over is a subslice of the input. An
    /// empty string from elsewhere is stored as an empty span at offset 0,
    /// which reads back as the same (empty) value.
    #[inline(always)]
    fn html_span(&self, value: &str) -> Span {
        let offset = value
            .as_ptr()
            .addr()
            .wrapping_sub(self.html.as_ptr().addr());
        if offset <= self.html.len() && value.len() <= self.html.len() - offset {
            // Both fit in u32: the HTML is shorter than u32::MAX bytes.
            Span {
                offset: offset as u32,
                len: value.len() as u32,
            }
        } else {
            assert!(
                value.is_empty(),
                "columnar store values must borrow from the parsed HTML"
            );
            Span { offset: 0, len: 0 }
        }
    }

    #[inline(always)]
    fn optional_html_span(&self, value: Option<&str>) -> Span {
        value.map_or(Span::ABSENT, |value| self.html_span(value))
    }

    /// Span of a text-tape range, or `None` if it does not fit in `u32`.
    #[inline(always)]
    fn tape_span(range: Range<usize>) -> Option<Span> {
        let len = range.end.checked_sub(range.start)?;
        (range.end < NONE as usize).then_some(Span {
            offset: range.start as u32,
            len: len as u32,
        })
    }

    /// Range of `attributes` within the attribute tape, mirroring
    /// `Arena::attribute_slice_to_range`.
    #[inline(always)]
    fn attribute_span(&self, attributes: &[Attribute<'html>]) -> Span {
        if self.attribute_tape.is_empty() || attributes.is_empty() {
            return Span::ABSENT;
        }
        let tape = self.attribute_tape.as_ptr_range();
        let start = attributes.as_ptr();
        assert!(
            tape.start <= start && start < tape.end,
            "attribute slice must belong to the persistent tape"
        );
        // SAFETY: `start` lies within the tape allocation (checked above),
        // and both pointers derive from that allocation.
        let offset = unsafe { start.offset_from_unsigned(tape.start) };
        let end = offset + attributes.len();
        assert!(end <= self.attribute_tape.len());
        // Each attribute spans at least one byte of the HTML, which is
        // shorter than u32::MAX bytes, so the tape length fits in u32.
        debug_assert!(end < NONE as usize);
        Span {
            offset: offset as u32,
            len: attributes.len() as u32,
        }
    }

    /// Switch on an optional column, back-filling absent values.
    #[cold]
    #[inline(never)]
    fn enable_column<T: Copy>(column: &mut Vec<T>, len: usize, absent: T) {
        column.resize(len, absent);
    }

    #[inline(always)]
    fn slice(&self, span: Span) -> &'html str {
        debug_assert!(span.is_present());
        debug_assert!(self.html.get(span.range()).is_some());
        // SAFETY: element spans are only built by `html_span`, which checks
        // that the value lies within `html`. The value was a `&str` subslice
        // of `html`, so both ends fall on UTF-8 character boundaries.
        unsafe { self.html.get_unchecked(span.range()) }
    }

    /// Value at `id` of a column, or `None` if the column is off.
    ///
    /// `id` must come from an [`ElementRef`] or a result-list link, so it is
    /// below `len()`.
    #[inline(always)]
    fn cell<T: Copy>(&self, column: &[T], id: u32) -> Option<T> {
        debug_assert!(column.is_empty() || column.len() == self.len());
        debug_assert!((id as usize) < self.len());
        if column.is_empty() {
            None
        } else {
            // SAFETY: a column is either off (empty) or holds exactly one
            // value per element: `push` appends to every enabled column, and
            // enabling a column back-fills it to the current element count.
            // The store is immutable once parsed, and `id` names a recorded
            // element, so it is below `len()` and thus in bounds.
            Some(unsafe { *column.get_unchecked(id as usize) })
        }
    }

    #[inline(always)]
    fn optional_slice(&self, column: &[Span], id: u32) -> Option<&'html str> {
        self.column_span(column, id).map(|span| self.slice(span))
    }

    #[inline(always)]
    fn column_span(&self, column: &[Span], id: u32) -> Option<Span> {
        self.cell(column, id).filter(|span| span.is_present())
    }
}

impl<'html, 'query: 'html> ResultSink<'html, 'query> for ColumnarStore<'html, 'query> {
    fn push(
        &mut self,
        from: ElementId,
        selection: &QuerySection<'query>,
        element: XHtmlElement<'html>,
    ) -> ElementId {
        debug_assert!(!self.contiguous, "push after compact_result_lists");
        let index = self.names.len();
        if self.overflowed || index >= MAX_ELEMENTS {
            // Results are discarded once a limit is exceeded; keep handing
            // out ids so the parser can finish without special cases.
            self.overflowed = true;
            return ElementId(index);
        }
        assert!(from.is_null() || from.0 < index);
        let element_id = index as u32;

        self.names.push(self.html_span(element.name));
        let save_attributes = selection.save.attributes;
        if save_attributes && !self.plan.attributes {
            self.plan.attributes = true;
            Self::enable_column(&mut self.classes, index, Span::ABSENT);
            Self::enable_column(&mut self.ids, index, Span::ABSENT);
            Self::enable_column(&mut self.attributes, index, Span::ABSENT);
        }
        if self.plan.attributes {
            if save_attributes {
                self.classes.push(self.optional_html_span(element.class));
                self.ids.push(self.optional_html_span(element.id));
                self.attributes
                    .push(self.attribute_span(element.attributes));
            } else {
                self.classes.push(Span::ABSENT);
                self.ids.push(Span::ABSENT);
                self.attributes.push(Span::ABSENT);
            }
        }
        if self.plan.inner_html {
            self.inner_html.push(Span::ABSENT);
        }
        if self.plan.raw_text {
            self.raw_text.push(Span::ABSENT);
        }
        if self.plan.text {
            self.text.push(Span::ABSENT);
        }
        if self.plan.nested {
            self.first_child_query.push(NONE);
        }
        self.next_sibling.push(NONE);

        let selector = self.intern(selection.source);
        let head = if from.is_null() {
            if self.nodes.is_empty() { NONE } else { 0 }
        } else {
            self.first_child_query.get(from.0).copied().unwrap_or(NONE)
        };

        let mut node = head;
        let mut tail = NONE;
        while node != NONE {
            let row = &self.nodes[node as usize];
            if row.selector == selector {
                break;
            }
            tail = node;
            node = row.next;
        }

        if node != NONE {
            let row = &mut self.nodes[node as usize];
            let last = std::mem::replace(&mut row.last, element_id);
            debug_assert_eq!(self.next_sibling[last as usize], NONE);
            self.next_sibling[last as usize] = element_id;
        } else {
            // There are never more query nodes than elements.
            let new_node = self.nodes.len() as u32;
            self.nodes.push(QueryNodeRow {
                selector,
                next: NONE,
                first: element_id,
                last: element_id,
            });
            if tail != NONE {
                self.nodes[tail as usize].next = new_node;
            } else if !from.is_null() {
                if !self.plan.nested {
                    self.plan.nested = true;
                    Self::enable_column(&mut self.first_child_query, index + 1, NONE);
                }
                self.first_child_query[from.0] = new_node;
            }
        }

        ElementId(index)
    }

    fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    ) {
        if self.overflowed {
            return;
        }
        let index = element_id.index();
        assert!(index < self.len());

        if inner_html.is_some() && !self.plan.inner_html {
            self.plan.inner_html = true;
            Self::enable_column(&mut self.inner_html, self.names.len(), Span::ABSENT);
        }
        if self.plan.inner_html {
            self.inner_html[index] = self.optional_html_span(inner_html);
        }
        if let Some(range) = raw_text {
            let Some(span) = Self::tape_span(range) else {
                self.overflowed = true;
                return;
            };
            if !self.plan.raw_text {
                self.plan.raw_text = true;
                Self::enable_column(&mut self.raw_text, self.names.len(), Span::ABSENT);
            }
            self.raw_text[index] = span;
        }
        if let Some(range) = text {
            let Some(span) = Self::tape_span(range) else {
                self.overflowed = true;
                return;
            };
            if !self.plan.text {
                self.plan.text = true;
                Self::enable_column(&mut self.text, self.names.len(), Span::ABSENT);
            }
            self.text[index] = span;
        }
    }

    #[inline(always)]
    fn attribute_tape(&mut self) -> &mut Arena<Attribute<'html>, AttributeId> {
        &mut self.attribute_tape
    }

    #[inline(always)]
    fn attribute_count(&self) -> usize {
        self.attribute_tape.len()
    }

    #[inline(always)]
    fn text_tapes(&self) -> &TextStore {
        &self.text_tapes
    }

    #[inline(always)]
    fn text_tapes_mut(&mut self) -> &mut TextStore {
        &mut self.text_tapes
    }

    #[inline(always)]
    fn element_count(&self) -> usize {
        self.names.len()
    }

    #[inline(always)]
    fn query_node_count(&self) -> usize {
        self.nodes.len()
    }

    #[cfg(any(debug_assertions, test))]
    fn element_name(&self, element_id: ElementId) -> &'html str {
        // Ids handed out after an overflow have no row.
        self.names
            .get(element_id.index())
            .map_or("", |span| self.slice(*span))
    }

    /// The columnar store keeps no trace log; events still reach the
    /// `otel` exporter through `scah_trace!`.
    #[cfg(any(debug_assertions, test))]
    #[inline(always)]
    fn trace_event(&mut self, _event: crate::debug::TraceEvent<'html, 'query>) {}
}

impl fmt::Debug for ColumnarStore<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColumnarStore")
            .field("elements", &self.len())
            .field("query_nodes", &self.nodes.len())
            .field("selectors", &self.selectors)
            .field("attributes", &self.attribute_tape.len())
            .finish_non_exhaustive()
    }
}

/// A matched element in a [`ColumnarStore`].
///
/// A copyable handle (store reference plus `u32` id). Strings taken from the
/// HTML (`name`, `class_name`, `html_id`, `inner_html`, attribute values)
/// borrow from the HTML itself rather than from the store.
#[derive(Clone, Copy)]
pub struct ElementRef<'store, 'html, 'query> {
    store: &'store ColumnarStore<'html, 'query>,
    id: u32,
}

impl<'store, 'html, 'query: 'html> ElementRef<'store, 'html, 'query> {
    /// This element's id within its store.
    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The tag name (e.g. `"a"`).
    #[inline]
    pub fn name(&self) -> &'html str {
        let span = self.store.cell(&self.store.names, self.id);
        self.store
            .slice(span.expect("every recorded element has a name"))
    }

    /// The `class` attribute value, if present and saved.
    #[inline]
    pub fn class_name(&self) -> Option<&'html str> {
        self.store.optional_slice(&self.store.classes, self.id)
    }

    /// The HTML `id` attribute value, if present and saved.
    ///
    /// Named apart from [`ElementRef::id`], which is the store id.
    #[inline]
    pub fn html_id(&self) -> Option<&'html str> {
        self.store.optional_slice(&self.store.ids, self.id)
    }

    /// The raw HTML between the opening and closing tags, when saved.
    #[inline]
    pub fn inner_html(&self) -> Option<&'html str> {
        self.store.optional_slice(&self.store.inner_html, self.id)
    }

    /// Attributes other than the first valued `class` and `id`, when saved
    /// and non-empty.
    #[inline]
    pub fn attributes(&self) -> Option<&'store [Attribute<'html>]> {
        self.store
            .column_span(&self.store.attributes, self.id)
            .map(|span| &self.store.attribute_tape.deref()[span.range()])
    }

    /// Value of the first attribute named `key` (ASCII case-insensitive).
    /// `None` when absent or valueless.
    #[inline]
    pub fn attribute(&self, key: &str) -> Option<&'html str> {
        self.attributes()?
            .iter()
            .find(|attribute| attribute.key.eq_ignore_ascii_case(key))
            .and_then(|attribute| attribute.value)
    }

    /// Source-preserving descendant text, when saved.
    #[inline]
    pub fn raw_text(&self) -> Option<&'store str> {
        self.store
            .column_span(&self.store.raw_text, self.id)
            .map(|span| self.store.text_tapes.raw_text.slice(span.range()))
    }

    /// Normalized descendant text, when saved.
    #[inline]
    pub fn text(&self) -> Option<&'store str> {
        self.store
            .column_span(&self.store.text, self.id)
            .map(|span| self.store.text_tapes.text.slice(span.range()))
    }

    /// Whether a raw-text range was captured (possibly empty).
    #[inline]
    pub fn has_raw_text(&self) -> bool {
        self.store
            .column_span(&self.store.raw_text, self.id)
            .is_some()
    }

    /// Whether a normalized-text range was captured (possibly empty).
    #[inline]
    pub fn has_text(&self) -> bool {
        self.store.column_span(&self.store.text, self.id).is_some()
    }

    /// Elements matched by the nested query with this selector string under
    /// this element.
    pub fn get(&self, selector: &str) -> Option<ColumnarElements<'store, 'html, 'query>> {
        let head = self
            .store
            .cell(&self.store.first_child_query, self.id)
            .unwrap_or(NONE);
        if head == NONE {
            return None;
        }
        self.store
            .find_node(head, selector)
            .map(|node| self.store.list(node))
    }
}

impl fmt::Debug for ElementRef<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementRef")
            .field("id", &self.id)
            .field("name", &self.name())
            .field("class", &self.class_name())
            .field("html_id", &self.html_id())
            .finish_non_exhaustive()
    }
}

/// Elements of one query result list, in match order.
#[derive(Clone)]
pub struct ColumnarElements<'store, 'html, 'query> {
    store: &'store ColumnarStore<'html, 'query>,
    ids: std::slice::Iter<'store, u32>,
}

impl<'store, 'html, 'query: 'html> Iterator for ColumnarElements<'store, 'html, 'query> {
    type Item = ElementRef<'store, 'html, 'query>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let id = *self.ids.next()?;
        Some(ElementRef {
            store: self.store,
            id,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl<'html, 'query: 'html> ExactSizeIterator for ColumnarElements<'_, 'html, 'query> {}

impl<'html, 'query: 'html> DoubleEndedIterator for ColumnarElements<'_, 'html, 'query> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let id = *self.ids.next_back()?;
        Some(ElementRef {
            store: self.store,
            id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Query, Save, parse_columnar};

    fn columns(store: &ColumnarStore<'_, '_>) -> [usize; 9] {
        [
            store.names.len(),
            store.classes.len(),
            store.ids.len(),
            store.attributes.len(),
            store.inner_html.len(),
            store.raw_text.len(),
            store.text.len(),
            store.first_child_query.len(),
            // Links while parsing, list ids once compacted.
            store.next_sibling.len().max(store.list_ids.len()),
        ]
    }

    #[test]
    fn span_is_eight_bytes() {
        assert_eq!(size_of::<Span>(), 8);
    }

    #[test]
    fn name_only_allocates_only_name_and_link_columns() {
        let html = "<a id='x' class='y' href='z'>one</a><a>two</a>";
        let queries = [Query::all("a", Save::name_only()).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();

        assert_eq!(columns(&store), [2, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(store.classes.capacity(), 0);
        assert_eq!(store.inner_html.capacity(), 0);
        assert_eq!(store.text.capacity(), 0);
        let first = store.element(0).unwrap();
        assert_eq!(first.name(), "a");
        assert_eq!(first.class_name(), None);
        assert_eq!(first.html_id(), None);
        assert_eq!(first.attributes(), None);
        assert_eq!(first.inner_html(), None);
        assert_eq!(first.text(), None);
        assert!(!first.has_text());
    }

    #[test]
    fn saved_fields_get_columns() {
        let html = "<a id='x' class='y' href='z'>one</a>";
        let queries = [Query::all("a", Save::all()).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();

        assert_eq!(columns(&store), [1, 1, 1, 1, 1, 1, 1, 0, 1]);
        let a = store.element(0).unwrap();
        assert_eq!(a.html_id(), Some("x"));
        assert_eq!(a.class_name(), Some("y"));
        assert_eq!(a.attribute("HREF"), Some("z"));
        assert_eq!(a.inner_html(), Some("one"));
        assert_eq!(a.raw_text(), Some("one"));
        assert_eq!(a.text(), Some("one"));
    }

    #[test]
    fn text_modes_get_independent_columns() {
        let html = "<p>A</p>";
        let only_text = Save::only_text().without_attributes();
        let queries = [Query::all("p", only_text).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();
        assert_eq!(columns(&store), [1, 0, 0, 0, 0, 0, 1, 0, 1]);

        let only_raw_text = Save::only_raw_text().without_attributes();
        let queries = [Query::all("p", only_raw_text).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();
        assert_eq!(columns(&store), [1, 0, 0, 0, 0, 1, 0, 0, 1]);
    }

    #[test]
    fn nested_queries_get_a_child_query_column() {
        let html = "<section><a>1</a></section><section></section>";
        let queries = [Query::all("section", Save::none())
            .unwrap()
            .then(|section| Ok([section.all("a", Save::none())?]))
            .unwrap()
            .build()];
        let store = parse_columnar(html, &queries).unwrap();

        // Node 0 lists the sections; node 1 lists the first section's links.
        assert_eq!(store.first_child_query, [1, NONE, NONE]);
        assert_eq!(store.list_ids, [0, 2, 1]);
        let sections: Vec<_> = store.get("section").unwrap().collect();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].get("a").unwrap().count(), 1);
        assert!(sections[1].get("a").is_none());
        assert!(sections[0].get("missing").is_none());
    }

    #[test]
    fn compaction_lays_lists_out_contiguously() {
        let html = "<section><a>1</a><a>2</a></section><section><a>3</a></section>";
        let queries = [Query::all("section", Save::none())
            .unwrap()
            .then(|section| Ok([section.all("a", Save::none())?]))
            .unwrap()
            .build()];
        let store = parse_columnar(html, &queries).unwrap();
        // Match order: section 0, a 1, a 2, section 3, a 4.
        assert!(store.contiguous);
        assert!(store.next_sibling.is_empty());
        assert_eq!(store.next_sibling.capacity(), 0);
        // Lists in creation order: sections, first links, second links.
        assert_eq!(store.list_ids, [0, 3, 1, 2, 4]);
        let ranges: Vec<_> = store.nodes.iter().map(|row| row.first..row.last).collect();
        assert_eq!(ranges, [0..2, 2..4, 4..5]);

        let links: Vec<Vec<u32>> = store
            .get("section")
            .unwrap()
            .map(|section| section.get("a").unwrap().map(|a| a.id()).collect())
            .collect();
        assert_eq!(links, [vec![1, 2], vec![4]]);
        assert_eq!(store.get("section").unwrap().len(), 2);
        let last = store.get("section").unwrap().next_back().unwrap();
        assert_eq!(last.id(), 3);
    }

    #[test]
    fn absent_and_empty_values_stay_distinct() {
        let html = r#"<a class="" href="">x</a><a>y</a><br>"#;
        let queries = [
            Query::all("a", Save::all()).unwrap().build(),
            Query::all("br", Save::only_text()).unwrap().build(),
        ];
        let store = parse_columnar(html, &queries).unwrap();
        let anchors: Vec<_> = store.get("a").unwrap().collect();

        assert_eq!(anchors[0].class_name(), Some(""));
        assert_eq!(anchors[0].attribute("href"), Some(""));
        assert_eq!(anchors[1].class_name(), None);
        assert_eq!(anchors[1].attributes(), None);
        let br = store.get("br").unwrap().next().unwrap();
        assert_eq!(br.text(), Some(""));
        assert!(br.has_text());
        assert_eq!(br.inner_html(), None);
    }

    #[test]
    fn valueless_attributes_have_no_value() {
        let html = "<input disabled type=text>";
        let queries = [Query::all("input", Save::all()).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();
        let input = store.element(0).unwrap();

        let attributes = input.attributes().unwrap();
        assert_eq!(attributes[0].key, "disabled");
        assert_eq!(attributes[0].value, None);
        assert_eq!(input.attribute("disabled"), None);
        assert_eq!(input.attribute("TYPE"), Some("text"));
    }

    #[test]
    fn element_lookup_is_bounded() {
        let html = "<a>1</a>";
        let queries = [Query::all("a", Save::none()).unwrap().build()];
        let store = parse_columnar(html, &queries).unwrap();

        assert_eq!(store.len(), 1);
        assert_eq!(store.element(0).unwrap().id(), 0);
        assert!(store.element(1).is_none());
        assert!(store.element(u32::MAX).is_none());
        assert!(store.get("b").is_none());
        assert_eq!(store.elements().len(), 1);
    }

    #[test]
    fn strings_borrow_from_the_html() {
        let html = String::from("<div id='a' class='b'><p>x</p></div>");
        let queries = [Query::all("div", Save::all()).unwrap().build()];
        let (name, inner) = {
            let store = parse_columnar(&html, &queries).unwrap();
            let div = store.element(0).unwrap();
            (div.name(), div.inner_html().unwrap())
        };
        // Element strings outlive the store; they borrow `html` directly.
        assert_eq!(name, "div");
        assert_eq!(inner, "<p>x</p>");
        let range = html.as_bytes().as_ptr_range();
        assert!(range.contains(&name.as_ptr()));
    }

    #[test]
    fn direct_pushes_back_fill_columns_off_in_the_plan() {
        let html = "<a href='x'>";
        let none = Query::all("a", Save::none()).unwrap().build();
        let all = Query::all("b", Save::all()).unwrap().build();
        let mut store = ColumnarStore::for_queries(html, std::slice::from_ref(&none), false);
        store.push(
            ElementId::default(),
            &none.queries[0],
            XHtmlElement {
                name: &html[1..2],
                ..Default::default()
            },
        );
        store.push(
            ElementId::default(),
            &all.queries[0],
            XHtmlElement {
                name: &html[1..2],
                id: Some(&html[9..10]),
                ..Default::default()
            },
        );
        store.set_content(ElementId(1), Some(""), None, Some(0..0));

        assert_eq!(columns(&store), [2, 2, 2, 2, 2, 0, 2, 0, 2]);
        assert_eq!(store.element(0).unwrap().html_id(), None);
        assert_eq!(store.element(1).unwrap().html_id(), Some("x"));
        assert_eq!(store.element(1).unwrap().text(), Some(""));
        assert_eq!(store.element(0).unwrap().text(), None);
    }

    #[test]
    fn heap_usage_counts_only_enabled_columns() {
        let html = "<a href='x'>1</a>".repeat(1_000);
        let queries = [Query::all("a", Save::name_only()).unwrap().build()];
        let store = parse_columnar(&html, &queries).unwrap();
        let usage = store.heap_usage();

        // A name span per reserved slot plus one list id per element.
        let slots = store.names.capacity();
        assert!(slots >= 1_000);
        assert_eq!(usage.elements, slots * 8 + store.len() * 4);
        assert_eq!(usage.attributes, 0);
        assert_eq!(usage.text_tapes, 0);

        let full = crate::parse(&html, &queries).unwrap().heap_usage();
        assert!(usage.total() < full.total());
    }

    #[test]
    fn oversized_text_ranges_mark_overflow() {
        let html = "<a>";
        let query = Query::all("a", Save::only_text()).unwrap().build();
        let mut store = ColumnarStore::for_queries(html, std::slice::from_ref(&query), false);
        store.push(
            ElementId::default(),
            &query.queries[0],
            XHtmlElement {
                name: &html[1..2],
                ..Default::default()
            },
        );
        store.set_content(ElementId(0), None, None, Some(0..NONE as usize));
        assert!(store.overflowed());
    }
}
