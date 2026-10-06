use std::fmt;

use super::columns::Columns;
use super::{Attribute, ElementId, Store};

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
        self.store
            .html_value(self.store.columns.rows[self.row.index()].name)
            .unwrap_or_default()
    }

    /// The `class` attribute value, if present and saved.
    #[inline]
    pub fn class(&self) -> Option<&'html str> {
        let cells = self.store.columns.attributes.get(self.row.index())?;
        self.store.html_value(cells.class)
    }

    /// The `id` attribute value, if present and saved.
    #[inline]
    pub fn id(&self) -> Option<&'html str> {
        let cells = self.store.columns.attributes.get(self.row.index())?;
        self.store.html_value(cells.id)
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
    pub fn attributes(&self) -> Option<&'store [Attribute<'html>]> {
        let cells = self.store.columns.attributes.get(self.row.index())?;
        cells
            .others
            .range()
            .map(|range| &self.store.attributes.as_slice()[range])
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
        self.attributes()?
            .iter()
            .find(|attribute| attribute.key.eq_ignore_ascii_case(key))
            .and_then(|attribute| attribute.value)
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
