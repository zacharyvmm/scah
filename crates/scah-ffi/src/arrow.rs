//! Zero-copy export of query results through the Apache Arrow C Data
//! Interface.
//!
//! An [`ArrowTable`] lays out the elements matched by one selector as
//! columns. String columns use the `Utf8View` layout: strings of up to 12
//! bytes are inlined in their 16-byte view, and longer strings point into the
//! store's own buffers (the HTML and the two text buffers), so building a
//! table copies no string data. Every exported struct keeps the table, and
//! through it the store and its HTML, alive until the consumer releases it.
//!
//! The struct definitions follow the [C Data Interface] and the
//! [C Stream Interface].
//!
//! [C Data Interface]: https://arrow.apache.org/docs/format/CDataInterface.html
//! [C Stream Interface]: https://arrow.apache.org/docs/format/CStreamInterface.html

use crate::OwnedStore;
use scah::{Element, Store};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::ptr;
use std::sync::Arc;

/// `ArrowSchema::flags` bit marking a field that may contain nulls.
pub const ARROW_FLAG_NULLABLE: i64 = 2;

/// The C Data Interface's `struct ArrowSchema`.
///
/// Released structs have a `None` release callback. Dropping a struct does
/// not release it.
#[repr(C)]
#[derive(Debug)]
pub struct ArrowSchema {
    pub format: *const c_char,
    pub name: *const c_char,
    pub metadata: *const c_char,
    pub flags: i64,
    pub n_children: i64,
    pub children: *mut *mut ArrowSchema,
    pub dictionary: *mut ArrowSchema,
    pub release: Option<unsafe extern "C" fn(*mut ArrowSchema)>,
    pub private_data: *mut c_void,
}

/// The C Data Interface's `struct ArrowArray`.
///
/// Released structs have a `None` release callback. Dropping a struct does
/// not release it.
#[repr(C)]
#[derive(Debug)]
pub struct ArrowArray {
    pub length: i64,
    pub null_count: i64,
    pub offset: i64,
    pub n_buffers: i64,
    pub n_children: i64,
    pub buffers: *mut *const c_void,
    pub children: *mut *mut ArrowArray,
    pub dictionary: *mut ArrowArray,
    pub release: Option<unsafe extern "C" fn(*mut ArrowArray)>,
    pub private_data: *mut c_void,
}

/// The C Stream Interface's `struct ArrowArrayStream`.
///
/// Released structs have a `None` release callback. Dropping a struct does
/// not release it.
#[repr(C)]
#[derive(Debug)]
pub struct ArrowArrayStream {
    pub get_schema: Option<unsafe extern "C" fn(*mut ArrowArrayStream, *mut ArrowSchema) -> c_int>,
    pub get_next: Option<unsafe extern "C" fn(*mut ArrowArrayStream, *mut ArrowArray) -> c_int>,
    pub get_last_error: Option<unsafe extern "C" fn(*mut ArrowArrayStream) -> *const c_char>,
    pub release: Option<unsafe extern "C" fn(*mut ArrowArrayStream)>,
    pub private_data: *mut c_void,
}

impl ArrowSchema {
    /// A released (empty) schema.
    pub const fn released() -> Self {
        Self {
            format: ptr::null(),
            name: ptr::null(),
            metadata: ptr::null(),
            flags: 0,
            n_children: 0,
            children: ptr::null_mut(),
            dictionary: ptr::null_mut(),
            release: None,
            private_data: ptr::null_mut(),
        }
    }
}

impl ArrowArray {
    /// A released (empty) array.
    pub const fn released() -> Self {
        Self {
            length: 0,
            null_count: 0,
            offset: 0,
            n_buffers: 0,
            n_children: 0,
            buffers: ptr::null_mut(),
            children: ptr::null_mut(),
            dictionary: ptr::null_mut(),
            release: None,
            private_data: ptr::null_mut(),
        }
    }
}

impl ArrowArrayStream {
    /// A released (empty) stream.
    pub const fn released() -> Self {
        Self {
            get_schema: None,
            get_next: None,
            get_last_error: None,
            release: None,
            private_data: ptr::null_mut(),
        }
    }
}

/// Why a table could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrowError {
    /// A requested attribute repeats a column name.
    DuplicateColumn(String),
    /// A requested attribute name contains a NUL byte.
    InvalidColumnName(String),
    /// A buffer or element id does not fit Arrow's 32-bit offsets.
    TooLarge(String),
}

impl fmt::Display for ArrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateColumn(name) => write!(f, "duplicate column name `{name}`"),
            Self::InvalidColumnName(name) => {
                write!(f, "column name {name:?} contains a NUL byte")
            }
            Self::TooLarge(detail) => write!(f, "too large for an Arrow export: {detail}"),
        }
    }
}

impl std::error::Error for ArrowError {}

/// Query results laid out as Arrow columns.
///
/// Columns, in order:
///
/// | Column | Type | Nulls |
/// |--------|------|-------|
/// | `index` | `uint32` | never; the element's id in its store |
/// | `parent` | `uint32` | never; only with a parent selector |
/// | `tag` | `Utf8View` | never |
/// | `inner_html`, `raw_text`, `text` | `Utf8View` | when not saved |
/// | one per requested attribute | `Utf8View` | when absent or valueless |
pub struct ArrowTable {
    store: Arc<OwnedStore>,
    num_rows: usize,
    columns: Vec<Column>,
    /// Copies of strings found in none of the store's buffers. Empty in
    /// practice; exported as a fourth data buffer only when non-empty.
    overflow: Vec<u8>,
}

struct Column {
    name: CString,
    data: ColumnData,
}

enum ColumnData {
    UInt32(Vec<u32>),
    Utf8View {
        views: Vec<View>,
        /// The shared buffers (`HTML`, `RAW_TEXT`, `TEXT`, `OVERFLOW`) that
        /// the views point into, in the order the views number them.
        data_buffers: Vec<usize>,
        /// LSB-ordered validity bitmap, `None` when the column has no nulls.
        validity: Option<Vec<u8>>,
        null_count: usize,
        nullable: bool,
    },
}

/// Column names that requested attributes may not reuse.
const FIXED_COLUMNS: [&str; 5] = ["index", "tag", "inner_html", "raw_text", "text"];

/// Indexes of the store's buffers among a view column's data buffers.
const HTML: usize = 0;
const RAW_TEXT: usize = 1;
const TEXT: usize = 2;
const OVERFLOW: usize = 3;

impl ArrowTable {
    /// Lay out the elements matched by top-level section `selector`, or, when
    /// `parent` is given, the elements matched by child section `selector`
    /// under every element matched by top-level section `parent`.
    ///
    /// `attributes` names extra columns. `id` and `class` read the element's
    /// dedicated fields; other names are looked up like
    /// [`Element::attribute`]. A selector without matches yields an empty
    /// table with the full schema.
    pub fn build(
        store: Arc<OwnedStore>,
        selector: &str,
        parent: Option<&str>,
        attributes: &[&str],
    ) -> Result<Self, ArrowError> {
        let names = column_names(parent.is_some(), attributes)?;
        let (columns, overflow, num_rows) = {
            let html = store.html().as_bytes();
            let doc = store.store();
            let mut buffers = Buffers::new([html, doc.raw_text_buffer(), doc.text_buffer()])?;
            let mut builder = Builder::new(parent.is_some(), attributes);
            match parent {
                None => {
                    for element in doc.get(selector).into_iter().flatten() {
                        builder.push(doc, None, element, &mut buffers)?;
                    }
                }
                Some(parent) => {
                    for parent in doc.get(parent).into_iter().flatten() {
                        let parent_index = element_index(doc, parent)?;
                        for element in parent.get(doc, selector).into_iter().flatten() {
                            builder.push(doc, Some(parent_index), element, &mut buffers)?;
                        }
                    }
                }
            }
            let num_rows = builder.index.len();
            (builder.finish(names), buffers.overflow, num_rows)
        };
        Ok(Self {
            store,
            num_rows,
            columns,
            overflow,
        })
    }

    /// The number of rows.
    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    /// The column names, in order.
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.columns
            .iter()
            .map(|column| column.name.to_str().expect("column names are UTF-8"))
    }

    /// Export the schema: a struct (record batch) with one field per column.
    pub fn export_schema(&self) -> ArrowSchema {
        let children = self
            .columns
            .iter()
            .map(|column| {
                let (format, flags) = match &column.data {
                    ColumnData::UInt32(_) => (c"I", 0),
                    ColumnData::Utf8View { nullable, .. } => {
                        (c"vu", if *nullable { ARROW_FLAG_NULLABLE } else { 0 })
                    }
                };
                new_schema(format, column.name.clone(), flags, Vec::new())
            })
            .collect();
        new_schema(c"+s", CString::default(), 0, children)
    }

    /// Export the data as one struct array of [`ArrowTable::num_rows`] rows.
    ///
    /// The array and each of its children hold a reference to the table.
    pub fn export_array(self: &Arc<Self>) -> ArrowArray {
        let children = self
            .columns
            .iter()
            .map(|column| self.export_column(column))
            .collect();
        // A struct array has a single buffer, its validity bitmap, which is
        // omitted because no row is null.
        new_array(
            self.clone(),
            self.num_rows,
            0,
            vec![ptr::null()],
            Vec::new(),
            children,
        )
    }

    /// Export the schema and the data together.
    pub fn export(self: &Arc<Self>) -> (ArrowSchema, ArrowArray) {
        (self.export_schema(), self.export_array())
    }

    /// Export a stream that yields the table as a single record batch.
    pub fn export_stream(self: &Arc<Self>) -> ArrowArrayStream {
        let private = Box::new(StreamPrivate {
            table: self.clone(),
            finished: false,
        });
        ArrowArrayStream {
            get_schema: Some(stream_get_schema),
            get_next: Some(stream_get_next),
            get_last_error: Some(stream_get_last_error),
            release: Some(release_stream),
            private_data: Box::into_raw(private).cast(),
        }
    }

    fn export_column(self: &Arc<Self>, column: &Column) -> ArrowArray {
        match &column.data {
            ColumnData::UInt32(values) => new_array(
                self.clone(),
                self.num_rows,
                0,
                vec![ptr::null(), values.as_ptr().cast()],
                Vec::new(),
                Vec::new(),
            ),
            ColumnData::Utf8View {
                views,
                data_buffers,
                validity,
                null_count,
                ..
            } => {
                let doc = self.store.store();
                // Only the buffers this column uses, so consumers that sum
                // buffer sizes don't count the HTML once per column.
                let data: Vec<&[u8]> = data_buffers
                    .iter()
                    .map(|&buffer| match buffer {
                        HTML => self.store.html().as_bytes(),
                        RAW_TEXT => doc.raw_text_buffer(),
                        TEXT => doc.text_buffer(),
                        _ => &self.overflow,
                    })
                    .collect();
                let variadic_sizes: Vec<i64> = data
                    .iter()
                    // Checked against i32::MAX when the table was built.
                    .map(|buffer| buffer.len() as i64)
                    .collect();
                let validity = validity.as_ref().map_or(ptr::null(), |bits| bits.as_ptr());
                let mut buffers: Vec<*const c_void> = Vec::with_capacity(data.len() + 3);
                buffers.push(validity.cast());
                buffers.push(views.as_ptr().cast());
                // Empty slices still have non-NULL (dangling, aligned)
                // pointers, which the spec allows for zero-sized buffers.
                buffers.extend(data.iter().map(|buffer| buffer.as_ptr().cast::<c_void>()));
                // The vector's heap buffer does not move when the vector is
                // moved into the private data below.
                buffers.push(variadic_sizes.as_ptr().cast());
                new_array(
                    self.clone(),
                    self.num_rows,
                    *null_count,
                    buffers,
                    variadic_sizes,
                    Vec::new(),
                )
            }
        }
    }
}

/// The table's column names, or an error when an attribute reuses one.
fn column_names(has_parent: bool, attributes: &[&str]) -> Result<Vec<CString>, ArrowError> {
    let mut names: Vec<&str> = vec!["index"];
    if has_parent {
        names.push("parent");
    }
    names.extend(&FIXED_COLUMNS[1..]);
    for &attribute in attributes {
        if names.contains(&attribute) {
            return Err(ArrowError::DuplicateColumn(attribute.to_owned()));
        }
        names.push(attribute);
    }
    names
        .into_iter()
        .map(|name| CString::new(name).map_err(|_| ArrowError::InvalidColumnName(name.to_owned())))
        .collect()
}

fn element_index(store: &Store<'_, '_>, element: &Element<'_>) -> Result<u32, ArrowError> {
    // SAFETY: callers only pass elements yielded by `store`'s lookups, which
    // live in its element arena (`index_of` also asserts this).
    let index = unsafe { store.elements.index_of(element) }.index();
    u32::try_from(index).map_err(|_| ArrowError::TooLarge(format!("element id {index}")))
}

/// Accumulates column values row by row.
struct Builder<'a> {
    index: Vec<u32>,
    parent: Option<Vec<u32>>,
    tag: ViewBuilder,
    inner_html: ViewBuilder,
    raw_text: ViewBuilder,
    text: ViewBuilder,
    attributes: Vec<(Attribute<'a>, ViewBuilder)>,
}

enum Attribute<'a> {
    Id,
    Class,
    Other(&'a str),
}

impl<'a> Builder<'a> {
    fn new(has_parent: bool, attributes: &[&'a str]) -> Self {
        Self {
            index: Vec::new(),
            parent: has_parent.then(Vec::new),
            tag: ViewBuilder::default(),
            inner_html: ViewBuilder::default(),
            raw_text: ViewBuilder::default(),
            text: ViewBuilder::default(),
            attributes: attributes
                .iter()
                .map(|&name| {
                    let attribute = if name.eq_ignore_ascii_case("id") {
                        Attribute::Id
                    } else if name.eq_ignore_ascii_case("class") {
                        Attribute::Class
                    } else {
                        Attribute::Other(name)
                    };
                    (attribute, ViewBuilder::default())
                })
                .collect(),
        }
    }

    fn push(
        &mut self,
        store: &Store<'_, '_>,
        parent: Option<u32>,
        element: &Element<'_>,
        buffers: &mut Buffers<'_>,
    ) -> Result<(), ArrowError> {
        self.index.push(element_index(store, element)?);
        if let (Some(parents), Some(parent)) = (&mut self.parent, parent) {
            parents.push(parent);
        }
        self.tag.push(Some(element.name), HTML, buffers)?;
        self.inner_html.push(element.inner_html, HTML, buffers)?;
        self.raw_text
            .push(element.raw_text(store), RAW_TEXT, buffers)?;
        self.text.push(element.text(store), TEXT, buffers)?;
        for (attribute, column) in &mut self.attributes {
            let value = match attribute {
                Attribute::Id => element.id,
                Attribute::Class => element.class,
                Attribute::Other(name) => element.attribute(store, name),
            };
            column.push(value, HTML, buffers)?;
        }
        Ok(())
    }

    fn finish(self, names: Vec<CString>) -> Vec<Column> {
        let mut data = vec![ColumnData::UInt32(self.index)];
        data.extend(self.parent.map(ColumnData::UInt32));
        data.push(self.tag.finish(false));
        data.push(self.inner_html.finish(true));
        data.push(self.raw_text.finish(true));
        data.push(self.text.finish(true));
        data.extend(
            self.attributes
                .into_iter()
                .map(|(_, column)| column.finish(true)),
        );
        names
            .into_iter()
            .zip(data)
            .map(|(name, data)| Column { name, data })
            .collect()
    }
}

/// One `Utf8View` view: a native-endian `i32` length, then either the
/// inlined bytes or a 4-byte prefix, buffer index, and offset.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct View([u8; 16]);

impl View {
    const MAX_INLINE: usize = 12;

    fn inline(value: &[u8]) -> Self {
        debug_assert!(value.len() <= Self::MAX_INLINE);
        let mut view = [0; 16];
        view[..4].copy_from_slice(&(value.len() as i32).to_ne_bytes());
        view[4..4 + value.len()].copy_from_slice(value);
        Self(view)
    }

    /// The buffer an out-of-line view points into.
    fn buffer(&self) -> Option<usize> {
        let len = i32::from_ne_bytes(self.0[..4].try_into().unwrap()) as usize;
        (len > Self::MAX_INLINE)
            .then(|| i32::from_ne_bytes(self.0[8..12].try_into().unwrap()) as usize)
    }

    fn set_buffer(&mut self, buffer: usize) {
        self.0[8..12].copy_from_slice(&(buffer as i32).to_ne_bytes());
    }

    /// `value` must be longer than 12 bytes; its length, `buffer`, and
    /// `offset` must fit an `i32`.
    fn reference(value: &[u8], buffer: usize, offset: usize) -> Self {
        let mut view = [0; 16];
        view[..4].copy_from_slice(&(value.len() as i32).to_ne_bytes());
        view[4..8].copy_from_slice(&value[..4]);
        view[8..12].copy_from_slice(&(buffer as i32).to_ne_bytes());
        view[12..].copy_from_slice(&(offset as i32).to_ne_bytes());
        Self(view)
    }
}

#[derive(Default)]
struct ViewBuilder {
    views: Vec<View>,
    validity: Vec<u8>,
    null_count: usize,
}

impl ViewBuilder {
    fn push(
        &mut self,
        value: Option<&str>,
        hint: usize,
        buffers: &mut Buffers<'_>,
    ) -> Result<(), ArrowError> {
        let row = self.views.len();
        if row.is_multiple_of(8) {
            self.validity.push(0);
        }
        match value {
            Some(value) => {
                self.views.push(buffers.view(value.as_bytes(), hint)?);
                self.validity[row / 8] |= 1 << (row % 8);
            }
            None => {
                self.views.push(View([0; 16]));
                self.null_count += 1;
            }
        }
        Ok(())
    }

    fn finish(mut self, nullable: bool) -> ColumnData {
        // Number the shared buffers this column uses consecutively.
        let mut used = [false; 4];
        for view in &self.views {
            if let Some(buffer) = view.buffer() {
                used[buffer] = true;
            }
        }
        let data_buffers: Vec<usize> = (0..used.len()).filter(|&b| used[b]).collect();
        let mut local = [0; 4];
        for (position, &buffer) in data_buffers.iter().enumerate() {
            local[buffer] = position;
        }
        for view in &mut self.views {
            if let Some(buffer) = view.buffer() {
                view.set_buffer(local[buffer]);
            }
        }
        ColumnData::Utf8View {
            views: self.views,
            data_buffers,
            validity: (self.null_count > 0).then_some(self.validity),
            null_count: self.null_count,
            nullable,
        }
    }
}

/// The buffers that views point into.
struct Buffers<'a> {
    shared: [&'a [u8]; 3],
    overflow: Vec<u8>,
}

impl<'a> Buffers<'a> {
    fn new(shared: [&'a [u8]; 3]) -> Result<Self, ArrowError> {
        for (buffer, name) in shared.iter().zip(["HTML", "raw text", "text"]) {
            if buffer.len() > i32::MAX as usize {
                return Err(ArrowError::TooLarge(format!(
                    "the {name} buffer has {} bytes",
                    buffer.len()
                )));
            }
        }
        Ok(Self {
            shared,
            overflow: Vec::new(),
        })
    }

    /// A view of `value`, which normally borrows from shared buffer `hint`.
    fn view(&mut self, value: &[u8], hint: usize) -> Result<View, ArrowError> {
        if value.len() <= View::MAX_INLINE {
            return Ok(View::inline(value));
        }
        for buffer in [hint, (hint + 1) % 3, (hint + 2) % 3] {
            if let Some(offset) = offset_in(self.shared[buffer], value) {
                return Ok(View::reference(value, buffer, offset));
            }
        }
        // Not borrowed from the store (for example a static string), so copy.
        let offset = self.overflow.len();
        if offset + value.len() > i32::MAX as usize {
            return Err(ArrowError::TooLarge(
                "copied strings exceed i32::MAX bytes".to_owned(),
            ));
        }
        self.overflow.extend_from_slice(value);
        Ok(View::reference(value, OVERFLOW, offset))
    }
}

/// The offset of `value` within `buffer`, if `value` lies inside it.
///
/// Compares addresses only. Two live allocations never overlap, so a
/// non-empty `value` whose address range falls within `buffer` is a
/// subslice of `buffer`.
fn offset_in(buffer: &[u8], value: &[u8]) -> Option<usize> {
    let offset = value.as_ptr().addr().checked_sub(buffer.as_ptr().addr())?;
    (offset <= buffer.len() && value.len() <= buffer.len() - offset).then_some(offset)
}

// ---------------------------------------------------------------------------
// Schema export

struct SchemaPrivate {
    name: CString,
    /// Child structs, which consumers may move out (leaving them released).
    children: Vec<ArrowSchema>,
    child_ptrs: Vec<*mut ArrowSchema>,
}

fn new_schema(
    format: &'static CStr,
    name: CString,
    flags: i64,
    mut children: Vec<ArrowSchema>,
) -> ArrowSchema {
    let child_ptrs: Vec<*mut ArrowSchema> = (0..children.len())
        // SAFETY: `i` is in bounds. `as_mut_ptr` does not create a reference
        // to the elements, so the pointers stay valid while the vector's heap
        // buffer is neither reallocated nor freed.
        .map(|i| unsafe { children.as_mut_ptr().add(i) })
        .collect();
    let mut private = Box::new(SchemaPrivate {
        name,
        children,
        child_ptrs,
    });
    ArrowSchema {
        format: format.as_ptr(),
        name: private.name.as_ptr(),
        metadata: ptr::null(),
        flags,
        n_children: private.child_ptrs.len() as i64,
        children: private.child_ptrs.as_mut_ptr(),
        dictionary: ptr::null_mut(),
        release: Some(release_schema),
        private_data: Box::into_raw(private).cast(),
    }
}

/// # Safety
///
/// `schema` must point to a live schema exported by [`new_schema`].
unsafe extern "C" fn release_schema(schema: *mut ArrowSchema) {
    // SAFETY: guaranteed by the caller.
    let schema = unsafe { &mut *schema };
    // SAFETY: `private_data` came from `Box::into_raw` in `new_schema` and is
    // reclaimed exactly once, because `release` is cleared below.
    let mut private = unsafe { Box::from_raw(schema.private_data.cast::<SchemaPrivate>()) };
    for child in &mut private.children {
        // Children the consumer moved out are already marked released.
        if let Some(release) = child.release {
            // SAFETY: the child is a live schema exported by `new_schema`.
            unsafe { release(child) };
        }
    }
    drop(private);
    *schema = ArrowSchema::released();
}

// ---------------------------------------------------------------------------
// Array export

struct ArrayPrivate {
    /// Keeps every buffer alive: the table's columns and, through the table,
    /// the store's HTML and text buffers.
    _table: Arc<ArrowTable>,
    buffers: Vec<*const c_void>,
    _variadic_sizes: Vec<i64>,
    /// Child structs, which consumers may move out (leaving them released).
    children: Vec<ArrowArray>,
    child_ptrs: Vec<*mut ArrowArray>,
}

fn new_array(
    table: Arc<ArrowTable>,
    length: usize,
    null_count: usize,
    buffers: Vec<*const c_void>,
    variadic_sizes: Vec<i64>,
    mut children: Vec<ArrowArray>,
) -> ArrowArray {
    let child_ptrs: Vec<*mut ArrowArray> = (0..children.len())
        // SAFETY: as in `new_schema`.
        .map(|i| unsafe { children.as_mut_ptr().add(i) })
        .collect();
    let mut private = Box::new(ArrayPrivate {
        _table: table,
        buffers,
        _variadic_sizes: variadic_sizes,
        children,
        child_ptrs,
    });
    ArrowArray {
        length: length as i64,
        null_count: null_count as i64,
        offset: 0,
        n_buffers: private.buffers.len() as i64,
        n_children: private.child_ptrs.len() as i64,
        buffers: private.buffers.as_mut_ptr(),
        children: private.child_ptrs.as_mut_ptr(),
        dictionary: ptr::null_mut(),
        release: Some(release_array),
        private_data: Box::into_raw(private).cast(),
    }
}

/// # Safety
///
/// `array` must point to a live array exported by [`new_array`].
unsafe extern "C" fn release_array(array: *mut ArrowArray) {
    // SAFETY: guaranteed by the caller.
    let array = unsafe { &mut *array };
    // SAFETY: `private_data` came from `Box::into_raw` in `new_array` and is
    // reclaimed exactly once, because `release` is cleared below.
    let mut private = unsafe { Box::from_raw(array.private_data.cast::<ArrayPrivate>()) };
    for child in &mut private.children {
        // Children the consumer moved out are already marked released.
        if let Some(release) = child.release {
            // SAFETY: the child is a live array exported by `new_array`.
            unsafe { release(child) };
        }
    }
    drop(private);
    *array = ArrowArray::released();
}

// ---------------------------------------------------------------------------
// Stream export

struct StreamPrivate {
    table: Arc<ArrowTable>,
    finished: bool,
}

/// # Safety
///
/// `stream` must point to a live stream exported by
/// [`ArrowTable::export_stream`].
unsafe fn stream_private<'a>(stream: *mut ArrowArrayStream) -> &'a mut StreamPrivate {
    // SAFETY: guaranteed by the caller; the private data lives until release.
    unsafe { &mut *(*stream).private_data.cast::<StreamPrivate>() }
}

/// # Safety
///
/// `stream` must be a live exported stream and `out` writable.
unsafe extern "C" fn stream_get_schema(
    stream: *mut ArrowArrayStream,
    out: *mut ArrowSchema,
) -> c_int {
    // SAFETY: guaranteed by the caller.
    unsafe {
        let schema = stream_private(stream).table.export_schema();
        out.write(schema);
    }
    0
}

/// # Safety
///
/// `stream` must be a live exported stream and `out` writable.
unsafe extern "C" fn stream_get_next(stream: *mut ArrowArrayStream, out: *mut ArrowArray) -> c_int {
    // SAFETY: guaranteed by the caller.
    unsafe {
        let private = stream_private(stream);
        let array = if private.finished {
            // A released array marks the end of the stream.
            ArrowArray::released()
        } else {
            private.finished = true;
            private.table.export_array()
        };
        out.write(array);
    }
    0
}

/// # Safety
///
/// Callable on any live exported stream. No call ever fails, so there is
/// never an error message.
unsafe extern "C" fn stream_get_last_error(_stream: *mut ArrowArrayStream) -> *const c_char {
    ptr::null()
}

/// # Safety
///
/// `stream` must point to a live stream exported by
/// [`ArrowTable::export_stream`].
unsafe extern "C" fn release_stream(stream: *mut ArrowArrayStream) {
    // SAFETY: guaranteed by the caller.
    let stream = unsafe { &mut *stream };
    // SAFETY: `private_data` came from `Box::into_raw` in `export_stream`
    // and is reclaimed exactly once, because `release` is cleared below.
    drop(unsafe { Box::from_raw(stream.private_data.cast::<StreamPrivate>()) });
    *stream = ArrowArrayStream::released();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OwnedQuery;
    use arrow_array::ffi::{FFI_ArrowArray, FFI_ArrowSchema, from_ffi};
    use arrow_array::ffi_stream::ArrowArrayStreamReader;
    use arrow_array::{
        Array, RecordBatch, RecordBatchReader, StringViewArray, StructArray, UInt32Array,
    };
    use arrow_schema::DataType;
    use scah::Save;
    use scah::lazy::{LazyQuery, LazyQueryBuilder};

    fn parse(html: &str, queries: Vec<LazyQueryBuilder<String>>) -> Arc<OwnedStore> {
        let queries: Vec<_> = queries
            .into_iter()
            .map(|query| OwnedQuery::build(query).unwrap())
            .collect();
        let queries: Vec<_> = queries.iter().collect();
        Arc::new(OwnedStore::parse(html.to_owned(), &queries).unwrap())
    }

    fn table(
        store: &Arc<OwnedStore>,
        selector: &str,
        parent: Option<&str>,
        attributes: &[&str],
    ) -> Arc<ArrowTable> {
        Arc::new(ArrowTable::build(store.clone(), selector, parent, attributes).unwrap())
    }

    /// Move an exported schema into arrow-rs, leaving `schema` released.
    fn import_schema(schema: &mut ArrowSchema) -> FFI_ArrowSchema {
        // SAFETY: `ArrowSchema` has the C layout that `FFI_ArrowSchema`
        // mirrors, and `from_raw` moves the struct out.
        unsafe { FFI_ArrowSchema::from_raw(ptr::from_mut(schema).cast()) }
    }

    /// Move an exported array into arrow-rs, leaving `array` released.
    fn import_array(array: &mut ArrowArray) -> FFI_ArrowArray {
        // SAFETY: as in `import_schema`.
        unsafe { FFI_ArrowArray::from_raw(ptr::from_mut(array).cast()) }
    }

    /// Import an exported table with arrow-rs, validating every buffer.
    fn import(table: &Arc<ArrowTable>) -> StructArray {
        let (mut schema, mut array) = table.export();
        let schema = import_schema(&mut schema);
        let array = import_array(&mut array);
        // SAFETY: both structs were just exported by `table`.
        let data = unsafe { from_ffi(array, &schema) }.unwrap();
        data.validate_full().unwrap();
        StructArray::from(data)
    }

    /// One row: the element id, the parent id, then tag, inner HTML, raw
    /// text, text, and the attributes.
    type Row = (u32, Option<u32>, Vec<Option<String>>);

    /// The rows the table should hold, read through the handle API.
    fn expected(
        store: &OwnedStore,
        selector: &str,
        parent: Option<&str>,
        attributes: &[&str],
    ) -> Vec<Row> {
        let doc = store.store();
        let id = |element| unsafe { doc.elements.index_of(element) }.index() as u32;
        let mut elements = Vec::new();
        match parent {
            None => elements.extend(doc.get(selector).into_iter().flatten().map(|e| (None, e))),
            Some(parent) => {
                for parent in doc.get(parent).into_iter().flatten() {
                    let children = parent.get(doc, selector).into_iter().flatten();
                    elements.extend(children.map(|e| (Some(id(parent)), e)));
                }
            }
        }
        elements
            .into_iter()
            .map(|(parent, element)| {
                let mut values = vec![
                    Some(element.name),
                    element.inner_html,
                    element.raw_text(doc),
                    element.text(doc),
                ];
                values.extend(attributes.iter().map(|&name| match name {
                    "id" => element.id,
                    "class" => element.class,
                    name => element.attribute(doc, name),
                }));
                let values = values.into_iter().map(|v| v.map(str::to_owned)).collect();
                (id(element), parent, values)
            })
            .collect()
    }

    fn strings(column: &dyn Array) -> &StringViewArray {
        column.as_any().downcast_ref().unwrap()
    }

    /// The rows of an imported table.
    fn rows(batch: &StructArray, has_parent: bool) -> Vec<Row> {
        let ids: &UInt32Array = batch.column(0).as_any().downcast_ref().unwrap();
        let parents: Option<&UInt32Array> =
            has_parent.then(|| batch.column(1).as_any().downcast_ref().unwrap());
        let first = 1 + usize::from(has_parent);
        (0..batch.len())
            .map(|row| {
                let values = batch.columns()[first..]
                    .iter()
                    .map(|column| {
                        let column = strings(column);
                        column.is_valid(row).then(|| column.value(row).to_owned())
                    })
                    .collect();
                (ids.value(row), parents.map(|p| p.value(row)), values)
            })
            .collect()
    }

    /// Check the schema, every cell, and that the views borrow the store's
    /// buffers. Returns the imported table.
    fn check(
        store: &Arc<OwnedStore>,
        selector: &str,
        parent: Option<&str>,
        attributes: &[&str],
    ) -> StructArray {
        let table = table(store, selector, parent, attributes);
        let batch = import(&table);

        let mut names = vec!["index"];
        names.extend(parent.map(|_| "parent"));
        names.extend(["tag", "inner_html", "raw_text", "text"]);
        names.extend(attributes);
        let fields = batch.fields();
        assert_eq!(fields.iter().map(|f| f.name()).collect::<Vec<_>>(), names);
        for field in fields.iter() {
            let nullable = !matches!(field.name().as_str(), "index" | "parent" | "tag");
            let data_type = match field.name().as_str() {
                "index" | "parent" => DataType::UInt32,
                _ => DataType::Utf8View,
            };
            assert_eq!(field.is_nullable(), nullable, "{field:?}");
            assert_eq!(field.data_type(), &data_type, "{field:?}");
        }

        let expected = expected(store, selector, parent, attributes);
        assert_eq!(table.num_rows(), expected.len());
        assert_eq!(rows(&batch, parent.is_some()), expected);

        let doc = store.store();
        let shared = [
            store.html().as_bytes(),
            doc.raw_text_buffer(),
            doc.text_buffer(),
        ];
        for column in &batch.columns()[2 + usize::from(parent.is_some())..] {
            // Each data buffer is one of the store's own, never a copy.
            let buffers = strings(column).data_buffers();
            assert!(buffers.len() <= shared.len());
            for buffer in buffers.iter() {
                assert!(
                    shared
                        .iter()
                        .any(|original| buffer.as_ptr() == original.as_ptr()
                            && buffer.len() == original.len()),
                    "copied a buffer"
                );
            }
        }
        batch
    }

    const LINKS: &str = concat!(
        "<ul id='list' class='links'>",
        "<li><a id='first' class='link external' title='' ",
        "href='https://example.com/a/very/long/path'>Fish &amp; chips are great</a></li>",
        "<li><a href='/b' disabled name='twelve-bytes' title='thirteen-byte'>B</a></li>",
        "<li><a href='/é' class=''>héllo wörld ✓ 日本語のテキスト &lt;tag&gt;</a></li>",
        "<li><a>  <b>nested</b>   text  spread   over   many  spaces  </a></li>",
        "<li><a href='/x'></a></li>",
        "</ul>",
    );

    const ATTRIBUTES: &[&str] = &["id", "class", "href", "name", "title", "disabled"];

    #[test]
    fn exports_every_value_of_the_handle_api() {
        let store = parse(
            LINKS,
            vec![
                LazyQuery::all("a".to_owned(), Save::all()),
                LazyQuery::all("li".to_owned(), Save::only_text()),
            ],
        );
        let batch = check(&store, "a", None, ATTRIBUTES);
        assert_eq!(batch.len(), 5);

        let column = |name| strings(batch.column_by_name(name).unwrap());
        // Missing, empty, and valueless values.
        assert!(column("id").is_null(1));
        assert_eq!(column("class").value(2), "");
        assert!(column("class").is_valid(2));
        assert_eq!(column("title").value(0), "");
        assert!(column("disabled").is_null(1));
        assert!(column("href").is_null(3));
        // Inline (12 bytes) and out-of-line (13 bytes) views.
        assert_eq!(column("name").value(1), "twelve-bytes");
        assert_eq!(column("title").value(1), "thirteen-byte");
        // Entities are decoded only in `text`.
        assert_eq!(column("text").value(0), "Fish & chips are great");
        assert_eq!(column("raw_text").value(0), "Fish &amp; chips are great");
        assert_eq!(
            column("text").value(2),
            "héllo wörld ✓ 日本語のテキスト <tag>"
        );
        assert_eq!(column("text").value(4), "");
        assert!(column("text").is_valid(4));
        assert_eq!(column("tag").null_count(), 0);
        assert_eq!(column("text").null_count(), 0);

        // Unsaved content and attributes are null.
        let batch = check(&store, "li", None, &["id", "class", "href"]);
        assert_eq!(batch.len(), 5);
        for name in ["inner_html", "raw_text", "id", "class", "href"] {
            assert_eq!(batch.column_by_name(name).unwrap().null_count(), 5);
        }
        assert_eq!(batch.column_by_name("text").unwrap().null_count(), 0);
    }

    #[test]
    fn attribute_names_are_matched_case_insensitively() {
        let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let batch = import(&table(&store, "a", None, &["ID", "Class", "HREF"]));
        let first = 5;
        assert_eq!(strings(batch.column(first)).value(0), "first");
        assert_eq!(strings(batch.column(first + 1)).value(0), "link external");
        assert_eq!(strings(batch.column(first + 2)).value(1), "/b");
    }

    #[test]
    fn exports_child_sections_with_their_parents() {
        let html = concat!(
            "<section id='products'>",
            "<div class='product'><h1>Product #1 with a long name</h1>",
            "<span class='rating'>4/5</span></div>",
            "<div class='product'><h1>Short</h1></div>",
            "<div class='product'></div>",
            "<div class='product'><h1>Fourth &amp; last product</h1>",
            "<span class='rating'>5/5</span></div>",
            "</section>",
        );
        let store = parse(
            html,
            vec![
                LazyQuery::all("div.product".to_owned(), Save::none()).then(|product| {
                    [
                        product.first("h1".to_owned(), Save::only_text()),
                        product.all("span.rating".to_owned(), Save::all()),
                    ]
                }),
            ],
        );

        let products = check(&store, "div.product", None, &[]);
        assert_eq!(products.len(), 4);
        let product_ids: &UInt32Array = products.column(0).as_any().downcast_ref().unwrap();

        let titles = check(&store, "h1", Some("div.product"), &["class"]);
        assert_eq!(titles.len(), 3);
        let parents: &UInt32Array = titles.column(1).as_any().downcast_ref().unwrap();
        assert_eq!(
            parents.values().to_vec(),
            [
                product_ids.value(0),
                product_ids.value(1),
                product_ids.value(3)
            ]
        );
        assert_eq!(
            strings(titles.column_by_name("text").unwrap()).value(2),
            "Fourth & last product"
        );

        let ratings = check(&store, "span.rating", Some("div.product"), &["class"]);
        assert_eq!(ratings.len(), 2);

        // Child sections are not top-level, and vice versa.
        assert_eq!(check(&store, "h1", None, &[]).len(), 0);
        assert_eq!(check(&store, "div.product", Some("h1"), &[]).len(), 0);
    }

    #[test]
    fn unmatched_selectors_yield_empty_tables() {
        let store = parse(
            "<p>text</p>",
            vec![LazyQuery::all("a".to_owned(), Save::all())],
        );
        for (selector, parent) in [("a", None), ("never", None), ("a", Some("never"))] {
            let batch = check(&store, selector, parent, &["href"]);
            assert_eq!(batch.len(), 0);
            assert_eq!(batch.num_columns(), 6 + usize::from(parent.is_some()));
        }
    }

    #[test]
    fn rejects_conflicting_column_names() {
        let store = parse("<a></a>", vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let build = |parent, attributes: &[&str]| {
            ArrowTable::build(store.clone(), "a", parent, attributes).err()
        };
        for name in ["index", "tag", "inner_html", "raw_text", "text"] {
            assert_eq!(
                build(None, &[name]),
                Some(ArrowError::DuplicateColumn(name.to_owned()))
            );
        }
        assert_eq!(
            build(Some("p"), &["parent"]),
            Some(ArrowError::DuplicateColumn("parent".to_owned()))
        );
        assert_eq!(build(None, &["parent"]), None);
        assert_eq!(
            build(None, &["href", "href"]),
            Some(ArrowError::DuplicateColumn("href".to_owned()))
        );
        assert_eq!(build(None, &["href", "HREF"]), None);
        assert_eq!(
            build(None, &["a\0b"]),
            Some(ArrowError::InvalidColumnName("a\0b".to_owned()))
        );
    }

    #[test]
    fn exports_outlive_the_store_and_the_table() {
        let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let expected = expected(&store, "a", None, ATTRIBUTES);
        let table = table(&store, "a", None, ATTRIBUTES);
        drop(store);

        let first = import(&table);
        let second = import(&table);
        drop(table);
        assert_eq!(rows(&first, false), expected);
        assert_eq!(rows(&second, false), expected);
        // Both exports share the same buffers rather than copies.
        assert_eq!(
            strings(first.column(2)).data_buffers()[0].as_ptr(),
            strings(second.column(2)).data_buffers()[0].as_ptr()
        );
    }

    #[test]
    fn consumers_may_move_children_out() {
        let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let expected: Vec<_> = expected(&store, "a", None, &[])
            .into_iter()
            .map(|(_, _, values)| values[3].clone())
            .collect();
        let table = table(&store, "a", None, &[]);
        let (mut schema, mut array) = table.export();
        drop((store, table));

        // Move the `text` column's schema and array out of their parents, as
        // the spec allows, then release the parents.
        // SAFETY: both parents are live exports with five children.
        let (mut text_schema, mut text_array) = unsafe {
            let schema_slot = *schema.children.add(4);
            let array_slot = *array.children.add(4);
            let moved = (ptr::read(schema_slot), ptr::read(array_slot));
            (*schema_slot).release = None;
            (*array_slot).release = None;
            (schema.release.unwrap())(&mut schema);
            (array.release.unwrap())(&mut array);
            moved
        };
        assert!(schema.release.is_none() && array.release.is_none());

        let text_schema = import_schema(&mut text_schema);
        // SAFETY: the moved child is still a live export.
        let data = unsafe { from_ffi(import_array(&mut text_array), &text_schema) }.unwrap();
        data.validate_full().unwrap();
        let text = StringViewArray::from(data);
        let actual: Vec<_> = text.iter().map(|v| v.map(str::to_owned)).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn streams_one_batch() {
        let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let expected = expected(&store, "a", None, ATTRIBUTES);
        let table = table(&store, "a", None, ATTRIBUTES);
        let mut stream = table.export_stream();
        drop((store, table));

        // SAFETY: `ArrowArrayStream` has the C layout `FFI_ArrowArrayStream`
        // mirrors, and `from_raw` moves the stream out.
        let reader =
            unsafe { ArrowArrayStreamReader::from_raw(ptr::from_mut(&mut stream).cast()) }.unwrap();
        assert!(stream.release.is_none());
        let schema = reader.schema();
        assert_eq!(schema.fields().len(), 4 + 1 + ATTRIBUTES.len());
        let batches: Vec<RecordBatch> = reader.collect::<Result<_, _>>().unwrap();
        assert_eq!(batches.len(), 1);
        let batch = StructArray::from(batches[0].clone());
        batch.to_data().validate_full().unwrap();
        assert_eq!(rows(&batch, false), expected);
    }

    #[test]
    fn stream_can_be_released_unread() {
        let store = parse(LINKS, vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let mut stream = table(&store, "a", None, &[]).export_stream();
        // SAFETY: a live exported stream; released once.
        unsafe {
            let mut schema = ArrowSchema::released();
            assert_eq!((stream.get_schema.unwrap())(&mut stream, &mut schema), 0);
            assert!(stream_get_last_error(&mut stream).is_null());
            (schema.release.unwrap())(&mut schema);
            (stream.release.unwrap())(&mut stream);
        }
        assert!(stream.release.is_none());
    }

    #[test]
    fn columns_list_only_the_buffers_they_use() {
        let store = parse(
            "<a href='/a/long/enough/path'>some text long enough to leave the view</a>",
            vec![LazyQuery::all("a".to_owned(), Save::only_text())],
        );
        let table = Arc::new(ArrowTable::build(store, "a", None, &["href", "title"]).unwrap());
        let batch = import(&table);
        let buffers = |name: &str| {
            strings(batch.column_by_name(name).unwrap())
                .data_buffers()
                .len()
        };
        // Short values are inlined, so `tag` needs no data buffer at all.
        assert_eq!(buffers("tag"), 0);
        assert_eq!(buffers("href"), 1);
        assert_eq!(buffers("text"), 1);
        assert_eq!(buffers("raw_text"), 0);
        assert_eq!(buffers("title"), 0);
        assert_eq!(
            strings(batch.column_by_name("text").unwrap()).value(0),
            "some text long enough to leave the view"
        );
    }

    #[test]
    fn copies_strings_from_outside_the_store() {
        static OUTSIDE: &str = "a string that lives outside the store";
        let store = parse("<a></a>", vec![LazyQuery::all("a".to_owned(), Save::all())]);
        let doc = store.store();
        let mut buffers = Buffers::new([
            store.html().as_bytes(),
            doc.raw_text_buffer(),
            doc.text_buffer(),
        ])
        .unwrap();
        let mut column = ViewBuilder::default();
        for value in [OUTSIDE, "inline", &OUTSIDE[2..]] {
            column.push(Some(value), HTML, &mut buffers).unwrap();
        }
        let table = Arc::new(ArrowTable {
            store: store.clone(),
            num_rows: 3,
            columns: vec![Column {
                name: c"value".to_owned(),
                data: column.finish(false),
            }],
            overflow: buffers.overflow,
        });

        let batch = import(&table);
        let column = strings(batch.column(0));
        // Only the overflow buffer holds this column's out-of-line values.
        assert_eq!(column.data_buffers().len(), 1);
        assert_eq!(column.value(0), OUTSIDE);
        assert_eq!(column.value(1), "inline");
        assert_eq!(column.value(2), &OUTSIDE[2..]);
    }

    #[test]
    fn views_follow_the_layout() {
        let inline = View::inline(b"twelve-bytes");
        assert_eq!(i32::from_ne_bytes(inline.0[..4].try_into().unwrap()), 12);
        assert_eq!(&inline.0[4..], b"twelve-bytes");

        let reference = View::reference(b"thirteen-byte", 2, 7);
        let field = |range: std::ops::Range<usize>| {
            i32::from_ne_bytes(reference.0[range].try_into().unwrap())
        };
        assert_eq!(field(0..4), 13);
        assert_eq!(&reference.0[4..8], b"thir");
        assert_eq!(field(8..12), 2);
        assert_eq!(field(12..16), 7);
    }

    #[test]
    fn offsets_require_containment() {
        let buffer = b"0123456789abcdefghij";
        assert_eq!(offset_in(buffer, &buffer[3..17]), Some(3));
        assert_eq!(offset_in(buffer, &buffer[..]), Some(0));
        assert_eq!(offset_in(&buffer[..10], &buffer[5..15]), None);
        assert_eq!(offset_in(&buffer[10..], &buffer[2..15]), None);
    }

    #[test]
    fn tables_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ArrowTable>();
    }
}
