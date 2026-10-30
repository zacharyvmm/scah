//! The columns of an [`ArrowTable`](super::ArrowTable): `uint32` element
//! ids, and `Utf8View` strings over one of the store's buffers.

use super::ArrowError;
use scah::{Buffer, ElementId, Field, Store};
use std::ffi::CString;
use std::ops::Range;

pub(super) struct Column {
    pub(super) name: CString,
    pub(super) data: ColumnData,
}

pub(super) enum ColumnData {
    UInt32(Vec<u32>),
    Utf8View {
        views: Vec<View>,
        /// The store buffer the out-of-line views point into, as data
        /// buffer 0. `None` when every value is inlined (or null), so the
        /// column exports no data buffer.
        buffer: Option<Buffer>,
        /// LSB-ordered validity bitmap, `None` when the column has no nulls.
        validity: Option<Vec<u8>>,
        null_count: usize,
        nullable: bool,
    },
}

impl ColumnData {
    /// The ids of `rows`, which store rows number in `u32`.
    pub(super) fn ids(rows: &[ElementId]) -> Self {
        Self::UInt32(rows.iter().map(|row| row.index() as u32).collect())
    }

    /// `field` of each of `rows`, null where the row has none.
    pub(super) fn strings(
        store: &Store<'_, '_>,
        rows: &[ElementId],
        field: Field<'_>,
        nullable: bool,
    ) -> Result<Self, ArrowError> {
        let buffer = field.buffer();
        let bytes = store.buffer(buffer);
        let mut views = Vec::with_capacity(rows.len());
        let mut validity = vec![0u8; rows.len().div_ceil(8)];
        let mut null_count = 0;
        let mut out_of_line = false;
        for (index, &row) in rows.iter().enumerate() {
            match store.field_range(row, field) {
                Some(range) => {
                    out_of_line |= range.len() > View::MAX_INLINE;
                    views.push(View::new(bytes, range));
                    validity[index / 8] |= 1 << (index % 8);
                }
                None => {
                    views.push(View::NULL);
                    null_count += 1;
                }
            }
        }
        // Checked once here rather than per view: offsets and lengths of
        // out-of-line views are within the buffer.
        if out_of_line && bytes.len() > i32::MAX as usize {
            return Err(ArrowError::TooLarge(format!(
                "the {buffer:?} buffer has {} bytes",
                bytes.len()
            )));
        }
        Ok(Self::Utf8View {
            views,
            buffer: out_of_line.then_some(buffer),
            validity: (null_count > 0).then_some(validity),
            null_count,
            nullable,
        })
    }
}

/// One `Utf8View` view: a native-endian `i32` length, then either the
/// inlined bytes, zero-padded, or a 4-byte prefix, the data buffer index,
/// and the offset in that buffer.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct View(pub(super) [u8; 16]);

impl View {
    pub(super) const MAX_INLINE: usize = 12;

    /// The view of a null value.
    const NULL: Self = Self([0; 16]);

    /// The view of `buffer[range]`, pointing at `range.start` of data
    /// buffer 0 when longer than [`View::MAX_INLINE`] bytes. Lengths and
    /// offsets past `i32` are truncated, so callers reject such buffers.
    ///
    /// Built as one `u128` in registers: writing the fields of a stack array
    /// one by one and copying it out stalls on store forwarding.
    #[inline]
    pub(super) fn new(buffer: &[u8], range: Range<usize>) -> Self {
        // Byte `i` of the view is bits `8 * i..8 * i + 8` of these words.
        let word = |bytes: [u8; 4]| u128::from(u32::from_le_bytes(bytes));
        let len = range.len();
        let mut view = word((len as i32).to_ne_bytes());
        if len <= Self::MAX_INLINE {
            let data = match buffer.get(range.start..range.start + Self::MAX_INLINE) {
                // Load a fixed 12 bytes straight from the buffer and clear
                // those past the value, which spares a variable-length copy
                // per value.
                Some(window) => {
                    let (low, high) = window.split_at(8);
                    u128::from(u64::from_le_bytes(low.try_into().unwrap()))
                        | word(high.try_into().unwrap()) << 64
                }
                // Too close to the end of the buffer for a fixed window.
                None => {
                    let mut data = [0; 16];
                    data[..len].copy_from_slice(&buffer[range]);
                    u128::from_le_bytes(data)
                }
            };
            view |= (data & ((1 << (8 * len)) - 1)) << 32;
        } else {
            let prefix = buffer[range.start..range.start + 4].try_into().unwrap();
            view |= word(prefix) << 32;
            // Buffer index 0 (bits 64..96): a column has one data buffer.
            view |= word((range.start as i32).to_ne_bytes()) << 96;
        }
        Self(view.to_le_bytes())
    }
}
