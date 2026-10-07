use std::ops::Range;

use crate::Save;

/// Offset of a [`Span`] that holds no value.
const ABSENT: u32 = u32::MAX;

/// A `(offset, len)` byte span into the HTML or a text tape. An offset of
/// `u32::MAX` marks an absent value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    offset: u32,
    len: u32,
}

impl Span {
    pub(crate) const ABSENT: Self = Self {
        offset: ABSENT,
        len: 0,
    };

    /// Key of a row's `class`, in place of its name in the HTML.
    pub(crate) const CLASS_KEY: Self = Self {
        offset: ABSENT,
        len: 1,
    };

    /// Key of a row's `id`, in place of its name in the HTML.
    pub(crate) const ID_KEY: Self = Self {
        offset: ABSENT,
        len: 2,
    };

    /// Span at `offset` of `len` bytes; both must fit in `u32`, and
    /// `offset` must not be `u32::MAX`.
    #[inline(always)]
    pub(crate) fn new(offset: usize, len: usize) -> Self {
        debug_assert!(offset < ABSENT as usize && len <= u32::MAX as usize);
        Self {
            offset: offset as u32,
            len: len as u32,
        }
    }

    /// Span of `range`, or `None` if it does not fit in `u32`.
    #[inline]
    pub(crate) fn of(range: Range<usize>) -> Option<Self> {
        let offset = u32::try_from(range.start)
            .ok()
            .filter(|&offset| offset != ABSENT)?;
        let len = u32::try_from(range.end.checked_sub(range.start)?).ok()?;
        Some(Self { offset, len })
    }

    /// Length in bytes; 0 when absent.
    #[inline(always)]
    pub(crate) fn len(self) -> usize {
        self.len as usize
    }

    #[inline(always)]
    pub(crate) fn range(self) -> Option<Range<usize>> {
        (self.offset != ABSENT).then(|| {
            let start = self.offset as usize;
            start..start + self.len as usize
        })
    }
}

/// What every row has: its tag name, the section that saved it, and where
/// its attributes start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowHead {
    pub(crate) name: Span,
    pub(crate) section: u32,
    /// Index of the row's first attribute in [`Columns::attribute_keys`]
    /// and [`Columns::attribute_values`]. Its attributes end where the next
    /// row's start, so a row that saved none starts where the next does.
    pub(crate) attributes: u32,
}

/// The optional columns a parse fills: those of the fields some query
/// section saves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnPlan {
    pub(crate) inner_html: bool,
    pub(crate) raw_text: bool,
    pub(crate) text: bool,
}

impl ColumnPlan {
    pub(crate) fn add(&mut self, save: Save) {
        self.inner_html |= save.inner_html;
        self.raw_text |= save.raw_text;
        self.text |= save.text;
    }
}

/// One column per element field, one value per row, and the saved
/// attributes.
///
/// `rows` always has a value per row. An optional column is empty when
/// the plan leaves it off, and otherwise holds one value per row,
/// [`Span::ABSENT`] where the row's section does not save that field.
///
/// Attributes are two parallel columns of HTML spans, one value per
/// attribute. Each row's attributes are consecutive: first its `class` and
/// then its `id` (the first of each with a value, when saved), then the
/// others in source order. The `class` and `id` keys are
/// [`Span::CLASS_KEY`] and [`Span::ID_KEY`], so finding them compares no
/// strings. A value is [`Span::ABSENT`] for an attribute without one.
#[derive(Debug, Default)]
pub(crate) struct Columns {
    pub(crate) plan: ColumnPlan,
    /// Rows this parse asked to reserve. Reused buffers can hold more, which
    /// optional columns should not copy.
    pub(crate) reserved_rows: usize,
    pub(crate) rows: Vec<RowHead>,
    pub(crate) attribute_keys: Vec<Span>,
    pub(crate) attribute_values: Vec<Span>,
    pub(crate) inner_html: Vec<Span>,
    pub(crate) raw_text: Vec<Span>,
    pub(crate) text: Vec<Span>,
}

/// Equal when the plan and every column's values are, whatever was
/// reserved.
impl PartialEq for Columns {
    fn eq(&self, other: &Self) -> bool {
        self.plan == other.plan
            && self.rows == other.rows
            && self.attribute_keys == other.attribute_keys
            && self.attribute_values == other.attribute_values
            && self.inner_html == other.inner_html
            && self.raw_text == other.raw_text
            && self.text == other.text
    }
}

impl Columns {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Remove every row, the plan, and the reservation, keeping the
    /// allocations.
    pub(crate) fn clear(&mut self) {
        self.plan = ColumnPlan::default();
        self.reserved_rows = 0;
        self.rows.clear();
        self.attribute_keys.clear();
        self.attribute_values.clear();
        self.inner_html.clear();
        self.raw_text.clear();
        self.text.clear();
    }

    /// Reserve the rows this parse asked for in every optional column the
    /// plan uses.
    pub(crate) fn reserve_optional(&mut self) {
        let rows = self.reserved_rows;
        if self.plan.inner_html {
            self.inner_html.reserve(rows);
        }
        if self.plan.raw_text {
            self.raw_text.reserve(rows);
        }
        if self.plan.text {
            self.text.reserve(rows);
        }
    }

    /// Append a row with no content yet.
    #[inline]
    pub(crate) fn push(&mut self, head: RowHead) {
        self.rows.push(head);
        if self.plan.inner_html {
            self.inner_html.push(Span::ABSENT);
        }
        if self.plan.raw_text {
            self.raw_text.push(Span::ABSENT);
        }
        if self.plan.text {
            self.text.push(Span::ABSENT);
        }
    }

    /// Indexes of `row`'s attributes in the attribute columns.
    ///
    /// # Safety
    ///
    /// `row` must be below [`Columns::len`].
    #[inline(always)]
    pub(crate) unsafe fn attribute_range_unchecked(&self, row: usize) -> Range<usize> {
        debug_assert!(row < self.rows.len());
        // SAFETY: the caller guarantees `row` is in bounds.
        let start = unsafe { self.rows.get_unchecked(row) }.attributes as usize;
        let end = self
            .rows
            .get(row + 1)
            .map_or(self.attribute_keys.len(), |next| next.attributes as usize);
        // Each row starts at the attribute count when it was saved, so starts
        // never decrease and never pass the end of the columns.
        debug_assert!(start <= end && end <= self.attribute_keys.len());
        start..end
    }

    /// Key and value spans of the attribute at `index`.
    ///
    /// # Safety
    ///
    /// `index` must be below the number of saved attributes, as every index
    /// [`Columns::attribute_range_unchecked`] returns is.
    #[inline(always)]
    pub(crate) unsafe fn attribute_unchecked(&self, index: usize) -> (Span, Span) {
        debug_assert!(index < self.attribute_keys.len());
        debug_assert_eq!(self.attribute_keys.len(), self.attribute_values.len());
        // SAFETY: the caller guarantees `index` is in bounds, and both columns
        // gain one value per attribute together (`Store::save_attributes`).
        unsafe {
            (
                *self.attribute_keys.get_unchecked(index),
                *self.attribute_values.get_unchecked(index),
            )
        }
    }

    /// Value of `row` in an optional column, if the column is on and the
    /// row saved it.
    #[inline(always)]
    pub(crate) fn cell(column: &[Span], row: usize) -> Option<Range<usize>> {
        column.get(row).and_then(|span| span.range())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_reject_ranges_past_u32() {
        assert_eq!(Span::of(3..5).and_then(Span::range), Some(3..5));
        assert_eq!(Span::of(0..0).and_then(Span::range), Some(0..0));
        assert_eq!(Span::of(u32::MAX as usize..u32::MAX as usize), None);
        assert_eq!(Span::of(1..u32::MAX as usize + 2), None);
        assert_eq!(Span::ABSENT.range(), None);
    }

    #[test]
    fn rows_without_a_column_read_as_absent() {
        let mut columns = Columns {
            plan: ColumnPlan {
                text: true,
                ..ColumnPlan::default()
            },
            ..Columns::default()
        };
        let head = RowHead {
            name: Span::new(0, 1),
            section: 0,
            attributes: 0,
        };
        columns.push(head);
        assert_eq!(columns.len(), 1);
        // SAFETY: row 0 exists.
        assert_eq!(unsafe { columns.attribute_range_unchecked(0) }, 0..0);
        assert_eq!(Columns::cell(&columns.text, 0), None);
        assert_eq!(Columns::cell(&columns.inner_html, 0), None);
    }
}
