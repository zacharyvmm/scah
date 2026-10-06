use std::ops::Range;

use crate::Save;

/// Offset of a [`Span`] that holds no value.
const ABSENT: u32 = u32::MAX;

/// A `(offset, len)` byte span into the HTML or a text tape, or a range of
/// the attribute tape. An offset of `u32::MAX` marks an absent value.
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

    #[inline(always)]
    pub(crate) fn range(self) -> Option<Range<usize>> {
        (self.offset != ABSENT).then(|| {
            let start = self.offset as usize;
            start..start + self.len as usize
        })
    }
}

/// What every row has: its tag name and the section that saved it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowHead {
    pub(crate) name: Span,
    pub(crate) section: u32,
}

/// `class`, `id` and the other attributes of a row, saved together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AttributeCells {
    pub(crate) class: Span,
    pub(crate) id: Span,
    /// Range of the attribute tape.
    pub(crate) others: Span,
}

impl AttributeCells {
    pub(crate) const ABSENT: Self = Self {
        class: Span::ABSENT,
        id: Span::ABSENT,
        others: Span::ABSENT,
    };
}

/// The optional columns a parse fills: those of the fields some query
/// section saves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnPlan {
    pub(crate) attributes: bool,
    pub(crate) inner_html: bool,
    pub(crate) raw_text: bool,
    pub(crate) text: bool,
}

impl ColumnPlan {
    pub(crate) fn add(&mut self, save: Save) {
        self.attributes |= save.attributes;
        self.inner_html |= save.inner_html;
        self.raw_text |= save.raw_text;
        self.text |= save.text;
    }
}

/// One column per element field, one value per row.
///
/// `rows` always has a value per row. An optional column is empty when
/// the plan leaves it off, and otherwise holds one value per row,
/// [`Span::ABSENT`] where the row's section does not save that field.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Columns {
    pub(crate) plan: ColumnPlan,
    pub(crate) rows: Vec<RowHead>,
    pub(crate) attributes: Vec<AttributeCells>,
    pub(crate) inner_html: Vec<Span>,
    pub(crate) raw_text: Vec<Span>,
    pub(crate) text: Vec<Span>,
}

impl Columns {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Reserve as many rows in every optional column the plan uses as
    /// `rows` has room for.
    pub(crate) fn reserve_optional(&mut self) {
        let rows = self.rows.capacity();
        if self.plan.attributes {
            self.attributes.reserve(rows);
        }
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
    pub(crate) fn push(&mut self, head: RowHead, attributes: AttributeCells) {
        self.rows.push(head);
        if self.plan.attributes {
            self.attributes.push(attributes);
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
        };
        columns.push(head, AttributeCells::ABSENT);
        assert_eq!(columns.len(), 1);
        assert!(columns.attributes.is_empty());
        assert_eq!(Columns::cell(&columns.text, 0), None);
        assert_eq!(Columns::cell(&columns.inner_html, 0), None);
    }
}
