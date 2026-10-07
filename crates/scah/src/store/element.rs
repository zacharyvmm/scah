use std::fmt;

use std::iter::FusedIterator;
use std::ops::Range;

use super::columns::{Columns, Span};
use super::{ElementId, Store};
use crate::Attribute;

/// A matched HTML element: a handle to one row of a [`Store`].
///
/// Each row is one HTML tag that a query section saved. Its fields live in
/// the store's columns and are read through methods. Strings borrow from the
/// parsed HTML (`'html`) or from the store's text buffers (`'store`).
///
/// | Data | How to access |
/// |------|---------------|
/// | Tag name | [`element.name()`](ElementRef::name) |
/// | Class | [`element.class()`](ElementRef::class) |
/// | ID | [`element.id()`](ElementRef::id) |
/// | Inner HTML | [`element.inner_html()`](ElementRef::inner_html) |
/// | Text content | [`element.text()`](ElementRef::text) |
/// | Raw text | [`element.raw_text()`](ElementRef::raw_text) |
/// | All attributes | [`element.attributes()`](ElementRef::attributes) |
/// | Single attribute | [`element.attribute("href")`](ElementRef::attribute) |
/// | Nested query results | [`element.get("selector")`](ElementRef::get) or [`element.nested(index)`](ElementRef::nested) |
#[derive(Clone, Copy)]
pub struct ElementRef<'store, 'html, 'query> {
    store: &'store Store<'html, 'query>,
    /// Always a row of `store`: handles are only made for the rows a store
    /// lists, and a store does not change once borrowed.
    row: ElementId,
}

impl<'store, 'html, 'query: 'html> ElementRef<'store, 'html, 'query> {
    #[inline]
    pub(crate) fn new(store: &'store Store<'html, 'query>, row: ElementId) -> Self {
        debug_assert!(row.index() < store.len());
        Self { store, row }
    }

    /// This element's row in the store.
    #[inline]
    pub fn index(&self) -> ElementId {
        self.row
    }

    /// The tag name (e.g. `"a"`).
    #[inline]
    pub fn name(&self) -> &'html str {
        // SAFETY: `row` is a row of `store` (see the field).
        let head = unsafe { self.store.columns.rows.get_unchecked(self.row.index()) };
        self.store.html_value(head.name).unwrap_or_default()
    }

    /// The `class` attribute value, if present and saved.
    ///
    /// The first `class` attribute with a value, like in selector matching.
    #[inline]
    pub fn class(&self) -> Option<&'html str> {
        let range = self.attribute_range();
        if !self.marked(&range, range.start, Span::CLASS_KEY) {
            return None;
        }
        self.value_at(range.start)
    }

    /// The `id` attribute value, if present and saved.
    ///
    /// The first `id` attribute with a value, like in selector matching.
    #[inline]
    pub fn id(&self) -> Option<&'html str> {
        let layout = self.layout();
        if !layout.id {
            return None;
        }
        self.value_at(layout.range.start + usize::from(layout.class))
    }

    /// Indexes of this row's attributes in the attribute columns.
    #[inline]
    fn attribute_range(&self) -> Range<usize> {
        // SAFETY: `row` is a row of `store` (see the field).
        unsafe {
            self.store
                .columns
                .attribute_range_unchecked(self.row.index())
        }
    }

    /// Value of the attribute at `index`, which must be in
    /// [`ElementRef::attribute_range`].
    #[inline]
    fn value_at(&self, index: usize) -> Option<&'html str> {
        debug_assert!(self.attribute_range().contains(&index));
        // SAFETY: the row's attribute indexes are in bounds.
        let (_, value) = unsafe { self.store.columns.attribute_unchecked(index) };
        self.store.html_value(value)
    }

    /// Whether the saved attribute at `index` (in
    /// [`ElementRef::attribute_range`]) is named `name`, ASCII
    /// case-insensitively. Compares lengths, then exact bytes, before
    /// folding case.
    #[inline]
    fn key_is(&self, index: usize, name: &str) -> bool {
        debug_assert!(self.attribute_range().contains(&index));
        // SAFETY: the row's attribute indexes are in bounds.
        let (key, _) = unsafe { self.store.columns.attribute_unchecked(index) };
        if key.len() != name.len() {
            return false;
        }
        let key = self.store.html_value(key).unwrap_or_default();
        key == name || key.eq_ignore_ascii_case(name)
    }

    /// Whether the attribute at `index`, if in `range`, has the key
    /// `marker`.
    #[inline]
    fn marked(&self, range: &Range<usize>, index: usize, marker: Span) -> bool {
        // SAFETY: `range` is the row's attribute range, whose indexes are in
        // bounds, and `index` is checked to be in it first.
        range.contains(&index)
            && unsafe { self.store.columns.attribute_unchecked(index) }.0 == marker
    }

    /// Where the row's attributes are: its `class` is saved first, then its
    /// `id`, then the others.
    #[inline]
    fn layout(&self) -> AttributeLayout {
        let range = self.attribute_range();
        let class = self.marked(&range, range.start, Span::CLASS_KEY);
        let id = self.marked(&range, range.start + usize::from(class), Span::ID_KEY);
        AttributeLayout { range, class, id }
    }

    /// The raw HTML between the opening and closing tags.
    /// Only saved when [`Save::inner_html`](crate::Save::inner_html) is `true`.
    #[inline]
    pub fn inner_html(&self) -> Option<&'html str> {
        let span = *self.store.columns.inner_html.get(self.row.index())?;
        self.store.html_value(span)
    }

    /// Attributes other than `class` and `id`, which have their own
    /// methods.
    ///
    /// Returns `None` if the element had no other attributes, or they were
    /// not saved.
    #[inline]
    pub fn attributes(&self) -> Option<Attributes<'store, 'html, 'query>> {
        let range = self.layout().others();
        (!range.is_empty()).then_some(Attributes {
            store: self.store,
            range,
        })
    }

    /// Look up a single attribute value by name (ASCII case-insensitive).
    ///
    /// Returns `None` if the attribute is not present, has no value, or was
    /// not saved.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scah::{Query, Save, parse};
    ///
    /// let html = r#"<a href="https://example.com">Link</a>"#;
    /// let queries = &[Query::all("a", Save::all())
    ///     .expect("valid selector")
    ///     .build()];
    /// let store = parse(html, queries).expect("parse succeeds");
    ///
    /// let a = store.get("a").unwrap().next().unwrap();
    /// assert_eq!(a.attribute("href"), Some("https://example.com"));
    /// ```
    pub fn attribute(&self, key: &str) -> Option<&'html str> {
        let index = self
            .layout()
            .others()
            .find(|&index| self.key_is(index, key))?;
        self.value_at(index)
    }

    /// The element's source-preserving descendant text.
    ///
    /// Only saved when [`Save::raw_text`](crate::Save::raw_text) is `true`.
    #[inline]
    pub fn raw_text(&self) -> Option<&'store str> {
        Columns::cell(&self.store.columns.raw_text, self.row.index())
            .map(|range| self.store.text.raw_text.slice(range))
    }

    /// The element's normalized, human-readable descendant text.
    ///
    /// Returns the whitespace-trimmed, concatenated text nodes within this
    /// element. Only saved when [`Save::text`](crate::Save::text) is `true`.
    #[inline]
    pub fn text(&self) -> Option<&'store str> {
        Columns::cell(&self.store.columns.text, self.row.index())
            .map(|range| self.store.text.text.slice(range))
    }

    /// Get normalized descendant text using the former method name.
    ///
    /// This compatibility method has the new [`ElementRef::text`] semantics.
    /// It does not reproduce the byte output of the former text-content
    /// extractor.
    #[deprecated(since = "0.1.0", note = "use `ElementRef::text()`")]
    pub fn text_content(&self) -> Option<&'store str> {
        self.text()
    }

    /// Whether a raw-text range was captured.
    ///
    /// Distinguishes uncaptured content (`false`) from captured empty
    /// content (`true` with [`ElementRef::raw_text`] returning `Some("")`).
    #[inline]
    pub fn has_raw_text(&self) -> bool {
        Columns::cell(&self.store.columns.raw_text, self.row.index()).is_some()
    }

    /// Whether a normalized-text range was captured.
    ///
    /// Distinguishes uncaptured content (`false`) from captured empty
    /// content (`true` with [`ElementRef::text`] returning `Some("")`).
    #[inline]
    pub fn has_text(&self) -> bool {
        Columns::cell(&self.store.columns.text, self.row.index()).is_some()
    }

    /// Elements matched under this one by its first **nested query** (one
    /// added via [`QueryBuilder::then`](crate::QueryBuilder::then)) whose
    /// selector is `selector`.
    ///
    /// Returns `None` if there is no such nested query, or it matched
    /// nothing under this element.
    #[inline]
    pub fn get(&self, selector: &str) -> Option<Elements<'store, 'html, 'query>> {
        let ids = self.store.child_results(self.row, selector)?;
        Some(Elements::new(self.store, ids))
    }

    /// Elements matched under this one by its nested query at `index`, in
    /// the order the nested queries were declared.
    #[inline]
    pub fn nested(&self, index: usize) -> Option<Elements<'store, 'html, 'query>> {
        let ids = self.store.nested_results(self.row, index)?;
        Some(Elements::new(self.store, ids))
    }
}

impl PartialEq for ElementRef<'_, '_, '_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.store, other.store) && self.row == other.row
    }
}

impl fmt::Debug for ElementRef<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementRef")
            .field("index", &self.row.index())
            .field("name", &self.name())
            .field("class", &self.class())
            .field("id", &self.id())
            .field("attributes", &self.attributes())
            .field("inner_html", &self.inner_html())
            .field("raw_text", &self.raw_text())
            .field("text", &self.text())
            .finish()
    }
}

/// Iterator over result elements, in document order.
#[derive(Clone)]
pub struct Elements<'store, 'html, 'query> {
    store: &'store Store<'html, 'query>,
    ids: std::slice::Iter<'store, ElementId>,
}

impl<'store, 'html, 'query: 'html> Elements<'store, 'html, 'query> {
    #[inline]
    pub(crate) fn new(store: &'store Store<'html, 'query>, ids: &'store [ElementId]) -> Self {
        Self {
            store,
            ids: ids.iter(),
        }
    }
}

impl<'store, 'html, 'query: 'html> Iterator for Elements<'store, 'html, 'query> {
    type Item = ElementRef<'store, 'html, 'query>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let &row = self.ids.next()?;
        Some(ElementRef::new(self.store, row))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let &row = self.ids.nth(n)?;
        Some(ElementRef::new(self.store, row))
    }
}

impl<'html, 'query: 'html> DoubleEndedIterator for Elements<'_, 'html, 'query> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let &row = self.ids.next_back()?;
        Some(ElementRef::new(self.store, row))
    }
}

impl<'html, 'query: 'html> ExactSizeIterator for Elements<'_, 'html, 'query> {}

impl fmt::Debug for Elements<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A row's attribute indexes, and whether they start with its `class` and
/// `id`.
struct AttributeLayout {
    range: Range<usize>,
    class: bool,
    id: bool,
}

impl AttributeLayout {
    /// Indexes of the attributes other than the row's `class` and `id`.
    #[inline]
    fn others(&self) -> Range<usize> {
        self.range.start + usize::from(self.class) + usize::from(self.id)..self.range.end
    }
}

/// Iterator over an element's attributes other than `class` and `id`, in
/// source order. Returned by [`ElementRef::attributes`].
#[derive(Clone)]
pub struct Attributes<'store, 'html, 'query> {
    store: &'store Store<'html, 'query>,
    /// Part of one row's attribute range, so every index is in bounds.
    range: Range<usize>,
}

impl<'html, 'query: 'html> Attributes<'_, 'html, 'query> {
    /// The attribute at `index`, taken from `range`.
    #[inline]
    fn attribute(&self, index: usize) -> Attribute<'html> {
        // SAFETY: `index` comes from `range`, whose indexes are in bounds.
        let (key, value) = unsafe { self.store.columns.attribute_unchecked(index) };
        Attribute {
            key: self.store.html_value(key).unwrap_or_default(),
            value: self.store.html_value(value),
        }
    }
}

impl<'html, 'query: 'html> Iterator for Attributes<'_, 'html, 'query> {
    type Item = Attribute<'html>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(self.attribute(index))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let index = self.range.nth(n)?;
        Some(self.attribute(index))
    }
}

impl<'html, 'query: 'html> DoubleEndedIterator for Attributes<'_, 'html, 'query> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let index = self.range.next_back()?;
        Some(self.attribute(index))
    }
}

impl<'html, 'query: 'html> ExactSizeIterator for Attributes<'_, 'html, 'query> {}

impl<'html, 'query: 'html> FusedIterator for Attributes<'_, 'html, 'query> {}

/// Equal when both yield the same attributes, from any stores.
impl<'html, 'query: 'html> PartialEq for Attributes<'_, 'html, 'query> {
    fn eq(&self, other: &Self) -> bool {
        self.range.len() == other.range.len() && self.clone().eq(other.clone())
    }
}

impl fmt::Debug for Attributes<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
