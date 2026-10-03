//! Apache Arrow export of query results.

use crate::error::{ErrorSlot, Failure, Result, ScahError, ScahStatus, deref, guard, out};
use crate::store::ScahStore;
use crate::string::ScahStringView;
use scah_ffi::arrow::{ArrowArray, ArrowError, ArrowSchema, ArrowTable};
use std::sync::Arc;

impl From<ArrowError> for Failure {
    fn from(err: ArrowError) -> Self {
        let status = match err {
            ArrowError::DuplicateColumn(_) | ArrowError::InvalidColumnName(_) => {
                ScahStatus::InvalidArgument
            }
            ArrowError::TooLarge(_) => ScahStatus::TooLarge,
        };
        Failure::new(status, err.to_string())
    }
}

/// Export matched elements as an Arrow record batch through the Arrow C Data
/// Interface.
///
/// Rows are the elements matched by top-level section `selector`, or, when
/// `parent.data` is not NULL, the elements matched by child section
/// `selector` under each element matched by top-level section `parent`.
///
/// `*out_schema` receives a struct schema (format `"+s"`) and `*out_array` a
/// struct array with these columns, in order:
///
/// - `index`: uint32 (`"I"`), the element's `ScahElementId`.
/// - `parent`: uint32, the parent's `ScahElementId`. Only with a `parent`.
/// - `tag`: Utf8View (`"vu"`), the tag name.
/// - `inner_html`, `raw_text`, `text`: nullable Utf8View, null unless the
///   query saved them.
/// - One nullable Utf8View column per entry of `attributes`, named after it
///   and looked up like `scah_element_attribute`; `id` and `class` read the
///   element's id and class. Null when absent, valueless, or not saved.
///
/// Strings of up to 12 bytes are inlined in their views; longer ones point
/// into the store's HTML and text buffers, which are exported as data
/// buffers without copying. The schema and the array each keep that data
/// alive until released through their `release` callbacks, even after
/// `scah_store_free`. A selector without matches yields an empty array.
///
/// On entry `*out_schema` and `*out_array` are marked released (`release`
/// set to NULL); on success both must be released by the caller.
/// `SCAH_STATUS_INVALID_ARGUMENT` reports an attribute that repeats a column
/// name or contains a NUL byte, and `SCAH_STATUS_TOO_LARGE` a buffer beyond
/// Arrow's 32-bit offsets.
///
/// # Safety
///
/// `store` must be NULL or a live store. `selector` must be a readable view,
/// and `parent` a readable view or one with NULL `data`. `attributes` must be
/// NULL with `attribute_count` 0, or point to `attribute_count` readable
/// views. `out_schema`, `out_array`, and `out_error` must be NULL or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_store_export_arrow(
    store: *const ScahStore,
    selector: ScahStringView,
    parent: ScahStringView,
    attributes: *const ScahStringView,
    attribute_count: usize,
    out_schema: *mut ArrowSchema,
    out_array: *mut ArrowArray,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        let out_schema = out(out_schema, "out_schema")?;
        let out_array = out(out_array, "out_array")?;
        // SAFETY: checked non-NULL above; the caller guarantees writable.
        unsafe {
            out_schema.write(ArrowSchema::released());
            out_array.write(ArrowArray::released());
        }
        // SAFETY: the caller passes NULL or a live store.
        let store = unsafe { deref(store, "store") }?;
        // SAFETY: the caller passes readable views.
        let selector = unsafe { selector.to_str("selector") }?;
        let parent = match parent.data.is_null() {
            true => None,
            // SAFETY: the caller passes a readable view.
            false => Some(unsafe { parent.to_str("parent") }?),
        };
        let attributes = match (attributes.is_null(), attribute_count) {
            (_, 0) => &[][..],
            (true, _) => return Err(Failure::null("attributes")),
            // SAFETY: the caller guarantees `attribute_count` readable views.
            (false, count) => unsafe { std::slice::from_raw_parts(attributes, count) },
        };
        let attributes = attributes
            .iter()
            .enumerate()
            // SAFETY: the caller passes readable views.
            .map(|(index, name)| unsafe { name.to_str(&format!("attributes[{index}]")) })
            .collect::<Result<Vec<_>>>()?;

        let table = ArrowTable::build(store.inner.clone(), selector, parent, &attributes)?;
        let (schema, array) = Arc::new(table).export();
        // SAFETY: checked non-NULL above; the caller guarantees writable.
        unsafe {
            out_schema.write(schema);
            out_array.write(array);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::tests::take_message;
    use crate::query::tests::{ALL, build, builder};
    use crate::store::scah_store_free;
    use crate::store::tests::parse;
    use std::ffi::CStr;
    use std::ptr;

    const NO_PARENT: ScahStringView = ScahStringView {
        data: ptr::null(),
        len: 0,
    };

    fn export(
        store: *const ScahStore,
        parent: ScahStringView,
        attributes: &[ScahStringView],
    ) -> (ScahStatus, ArrowSchema, ArrowArray, Option<String>) {
        let mut schema = ArrowSchema::released();
        let mut array = ArrowArray::released();
        let mut error = ptr::null_mut();
        // SAFETY: the views borrow live strings and the outputs are locals.
        let status = unsafe {
            scah_store_export_arrow(
                store,
                ScahStringView::new("a"),
                parent,
                attributes.as_ptr(),
                attributes.len(),
                &mut schema,
                &mut array,
                &mut error,
            )
        };
        let message = (!error.is_null()).then(|| take_message(error));
        (status, schema, array, message)
    }

    #[test]
    fn exports_outlive_the_store() {
        let store = parse(
            "<p><a href='/a/long/enough/link'>x</a></p>",
            &[build(builder("a", ALL))],
        );
        let (status, mut schema, mut array, _) =
            export(store, NO_PARENT, &[ScahStringView::new("href")]);
        assert_eq!(status, ScahStatus::Ok);
        // SAFETY: the store is live and owned by the test.
        unsafe { scah_store_free(store) };

        // SAFETY: both exports are live; their children are valid arrays.
        unsafe {
            assert_eq!(CStr::from_ptr(schema.format), c"+s");
            assert_eq!(schema.n_children, 6);
            assert_eq!(array.length, 1);
            let href = &**array.children.add(5);
            let view = std::slice::from_raw_parts(href.buffers.add(1).read().cast::<u8>(), 16);
            let [len, _, buffer, offset] =
                [0, 4, 8, 12].map(|at| i32::from_ne_bytes(view[at..at + 4].try_into().unwrap()));
            assert_eq!((len, buffer), (19, 0));
            let html = href.buffers.add(2).read().cast::<u8>();
            let value = std::slice::from_raw_parts(html.add(offset as usize), len as usize);
            assert_eq!(value, b"/a/long/enough/link");

            (schema.release.unwrap())(&mut schema);
            (array.release.unwrap())(&mut array);
        }
        assert!(schema.release.is_none() && array.release.is_none());
    }

    #[test]
    fn errors_leave_outputs_released() {
        let store = parse("<a></a>", &[build(builder("a", ALL))]);
        let (status, schema, array, message) =
            export(store, NO_PARENT, &[ScahStringView::new("tag")]);
        assert_eq!(status, ScahStatus::InvalidArgument);
        assert_eq!(message.as_deref(), Some("duplicate column name `tag`"));
        assert!(schema.release.is_none() && array.release.is_none());

        let (status, ..) = export(
            store,
            ScahStringView::new("section"),
            &[ScahStringView::new("parent")],
        );
        assert_eq!(status, ScahStatus::InvalidArgument);

        let invalid = [ScahStringView {
            data: [0xff].as_ptr(),
            len: 1,
        }];
        let (status, _, _, message) = export(store, NO_PARENT, &invalid);
        assert_eq!(status, ScahStatus::InvalidUtf8);
        assert!(
            message
                .unwrap()
                .starts_with("`attributes[0]` is not valid UTF-8")
        );

        let (status, _, _, message) = export(ptr::null(), NO_PARENT, &[]);
        assert_eq!(status, ScahStatus::NullPointer);
        assert_eq!(message.as_deref(), Some("`store` must not be NULL"));

        let mut schema = ArrowSchema::released();
        // SAFETY: NULL arguments are rejected before use.
        unsafe {
            let status = scah_store_export_arrow(
                store,
                ScahStringView::new("a"),
                NO_PARENT,
                ptr::null(),
                1,
                &mut schema,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert_eq!(status, ScahStatus::NullPointer);
            let mut array = ArrowArray::released();
            let status = scah_store_export_arrow(
                store,
                ScahStringView::new("a"),
                NO_PARENT,
                ptr::null(),
                1,
                &mut schema,
                &mut array,
                ptr::null_mut(),
            );
            assert_eq!(status, ScahStatus::NullPointer);
            scah_store_free(store);
        }
    }

    #[test]
    fn too_large_maps_to_its_status() {
        let failure = Failure::from(ArrowError::TooLarge("x".to_owned()));
        assert_eq!(failure.status, ScahStatus::TooLarge);
    }
}
