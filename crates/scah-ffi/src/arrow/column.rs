//! The columns of an [`ArrowTable`](super::ArrowTable): `uint32` element
//! ids, and `Utf8View` strings over the store's buffers.

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
        /// The store buffers the out-of-line views point into, as data
        /// buffers in this order: none when every value is inlined (or
        /// null), and at most two, for attribute values in the HTML and
        /// decoded ones.
        buffers: Vec<Buffer>,
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
        let mut views = Vec::with_capacity(rows.len());
        let mut validity = vec![0u8; rows.len().div_ceil(8)];
        let mut null_count = 0;
        let mut buffers = Vec::new();
        for (index, &row) in rows.iter().enumerate() {
            match store.field_range(row, field) {
                Some((buffer, range)) => {
                    let data_buffer = if range.len() > View::MAX_INLINE {
                        data_buffer_index(&mut buffers, buffer)
                    } else {
                        0
                    };
                    views.push(View::new(store.buffer(buffer), range, data_buffer));
                    validity[index / 8] |= 1 << (index % 8);
                }
                None => {
                    views.push(View::NULL);
                    null_count += 1;
                }
            }
        }
        // Checked once here rather than per view: offsets and lengths of
        // out-of-line views are within their buffer.
        for &buffer in &buffers {
            let len = store.buffer(buffer).len();
            if len > i32::MAX as usize {
                return Err(ArrowError::TooLarge(format!(
                    "the {buffer:?} buffer has {len} bytes"
                )));
            }
        }
        Ok(Self::Utf8View {
            views,
            buffers,
            validity: (null_count > 0).then_some(validity),
            null_count,
            nullable,
        })
    }
}

/// The index of `buffer` among a column's data `buffers`, added if new.
#[inline]
fn data_buffer_index(buffers: &mut Vec<Buffer>, buffer: Buffer) -> u32 {
    let index = match buffers.iter().position(|&known| known == buffer) {
        Some(index) => index,
        None => {
            buffers.push(buffer);
            buffers.len() - 1
        }
    };
    // A column's values are in at most two buffers.
    index as u32
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
    /// buffer `data_buffer` when longer than [`View::MAX_INLINE`] bytes.
    /// Lengths and offsets past `i32` are truncated, so callers reject such
    /// buffers.
    ///
    /// Built as one `u128` in registers: writing the fields of a stack array
    /// one by one and copying it out stalls on store forwarding.
    #[inline]
    pub(super) fn new(buffer: &[u8], range: Range<usize>, data_buffer: u32) -> Self {
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
            view |= word((data_buffer as i32).to_ne_bytes()) << 64;
            view |= word((range.start as i32).to_ne_bytes()) << 96;
        }
        Self(view.to_le_bytes())
    }
}
