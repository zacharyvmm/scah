//! The [C Data Interface] and [C Stream Interface] structs, and exporting a
//! table through them.
//!
//! Every exported struct owns its children and a reference to the table,
//! and frees both in its release callback. Consumers may move children out,
//! leaving them marked released, as the spec allows.
//!
//! [C Data Interface]: https://arrow.apache.org/docs/format/CDataInterface.html
//! [C Stream Interface]: https://arrow.apache.org/docs/format/CStreamInterface.html

use super::ArrowTable;
use super::column::{Column, ColumnData};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
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

/// The schema of `columns`: a struct (record batch) with one field each.
pub(super) fn export_schema(columns: &[Column]) -> ArrowSchema {
    let children = columns
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

/// The data of `table` as one struct array.
pub(super) fn export_array(table: &Arc<ArrowTable>) -> ArrowArray {
    let children = table
        .columns
        .iter()
        .map(|column| export_column(table, &column.data))
        .collect();
    // A struct array has a single buffer, its validity bitmap, which is
    // omitted because no row is null.
    new_array(
        table.clone(),
        table.num_rows,
        0,
        vec![ptr::null()],
        Vec::new(),
        children,
    )
}

fn export_column(table: &Arc<ArrowTable>, data: &ColumnData) -> ArrowArray {
    match data {
        ColumnData::UInt32(values) => new_array(
            table.clone(),
            table.num_rows,
            0,
            vec![ptr::null(), values.as_ptr().cast()],
            Vec::new(),
            Vec::new(),
        ),
        ColumnData::Utf8View {
            views,
            buffer,
            validity,
            null_count,
            ..
        } => {
            // At most the one store buffer this column's views point into.
            let data: Option<&[u8]> = buffer.map(|buffer| table.store.store().buffer(buffer));
            // Checked against i32::MAX when the column was built.
            let variadic_sizes: Vec<i64> = data.iter().map(|data| data.len() as i64).collect();
            let validity = validity.as_ref().map_or(ptr::null(), |bits| bits.as_ptr());
            let mut buffers: Vec<*const c_void> = vec![validity.cast(), views.as_ptr().cast()];
            // An empty slice still has a non-NULL (dangling, aligned)
            // pointer, which the spec allows for a zero-sized buffer.
            buffers.extend(data.map(|data| data.as_ptr().cast::<c_void>()));
            // The vector's heap buffer does not move when the vector is moved
            // into the private data below.
            buffers.push(variadic_sizes.as_ptr().cast());
            new_array(
                table.clone(),
                table.num_rows,
                *null_count,
                buffers,
                variadic_sizes,
                Vec::new(),
            )
        }
    }
}

/// A stream that yields `table` as a single record batch.
pub(super) fn export_stream(table: Arc<ArrowTable>) -> ArrowArrayStream {
    let private = Box::new(StreamPrivate {
        table,
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
