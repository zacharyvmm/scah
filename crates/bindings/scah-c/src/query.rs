//! Query builders and compiled queries.

use crate::error::{
    ErrorSlot, Failure, Result, ScahError, ScahStatus, deref, guard, guard_value, out_handle,
};
use crate::string::ScahStringView;
use scah::Save;
use scah::lazy::{LazyQuery, LazyQueryBuilder};
use scah_ffi::OwnedQuery;

/// What to keep for each element a query section matches.
///
/// The tag name is always kept. Each field must be `true` or `false`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScahSave {
    /// Keep the HTML between the opening and closing tags.
    pub inner_html: bool,
    /// Keep the descendant text exactly as written in the source.
    pub raw_text: bool,
    /// Keep the descendant text with entities decoded and whitespace
    /// collapsed.
    pub text: bool,
    /// Keep the `id`, `class`, and other attributes.
    pub attributes: bool,
}

impl From<ScahSave> for Save {
    fn from(save: ScahSave) -> Self {
        Save {
            inner_html: save.inner_html,
            raw_text: save.raw_text,
            text: save.text,
            attributes: save.attributes,
        }
    }
}

/// A query being described: a chain of selector sections, each of which may
/// have nested child queries.
///
/// Created by `scah_query_builder_new_all` or `scah_query_builder_new_first`
/// and released with `scah_query_builder_free`. Selectors are only compiled
/// by `scah_query_build`.
pub struct ScahQueryBuilder {
    inner: LazyQueryBuilder<String>,
}

/// A compiled, immutable query.
///
/// Created by `scah_query_build` and released with `scah_query_free`. Stores
/// keep their own reference, so a query may be freed right after parsing.
pub struct ScahQuery {
    pub(crate) inner: OwnedQuery,
}

unsafe fn new_builder(
    first: bool,
    selector: ScahStringView,
    save: ScahSave,
    out_builder: *mut *mut ScahQueryBuilder,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_builder`.
        let out_builder = unsafe { out_handle(out_builder, "out_builder") }?;
        // SAFETY: the caller passes a readable selector view.
        let selector = unsafe { selector.to_str("selector") }?.to_owned();
        let inner = if first {
            LazyQuery::first(selector, save.into())
        } else {
            LazyQuery::all(selector, save.into())
        };
        let builder = Box::into_raw(Box::new(ScahQueryBuilder { inner }));
        // SAFETY: checked non-NULL and writable above.
        unsafe { out_builder.write(builder) };
        Ok(())
    })
}

unsafe fn push_section(
    first: bool,
    builder: *mut ScahQueryBuilder,
    selector: ScahStringView,
    save: ScahSave,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes NULL or a live, unaliased builder.
        let builder = unsafe { builder.as_mut() }.ok_or_else(|| Failure::null("builder"))?;
        // SAFETY: the caller passes a readable selector view.
        let selector = unsafe { selector.to_str("selector") }?.to_owned();
        if first {
            builder.inner.first_mut(selector, save.into());
        } else {
            builder.inner.all_mut(selector, save.into());
        }
        Ok(())
    })
}

/// Start a query whose first section matches every element selected by
/// `selector`.
///
/// On success `*out_builder` receives a builder to free with
/// `scah_query_builder_free`. The selector is copied and is only compiled by
/// `scah_query_build`.
///
/// # Safety
///
/// `selector` must be a readable view. `out_builder` and `out_error` must be
/// NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_new_all(
    selector: ScahStringView,
    save: ScahSave,
    out_builder: *mut *mut ScahQueryBuilder,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe { new_builder(false, selector, save, out_builder, out_error) }
}

/// Start a query whose first section matches only the first element selected
/// by `selector`, which lets parsing stop early.
///
/// Otherwise identical to `scah_query_builder_new_all`.
///
/// # Safety
///
/// Same as `scah_query_builder_new_all`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_new_first(
    selector: ScahStringView,
    save: ScahSave,
    out_builder: *mut *mut ScahQueryBuilder,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe { new_builder(true, selector, save, out_builder, out_error) }
}

/// Append a section that matches every element selected by `selector` inside
/// each match of the builder's last section.
///
/// # Safety
///
/// `builder` must be NULL or a live builder not in use elsewhere. `selector`
/// must be a readable view. `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_all(
    builder: *mut ScahQueryBuilder,
    selector: ScahStringView,
    save: ScahSave,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe { push_section(false, builder, selector, save, out_error) }
}

/// Append a section that matches only the first element selected by
/// `selector` inside each match of the builder's last section.
///
/// # Safety
///
/// Same as `scah_query_builder_all`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_first(
    builder: *mut ScahQueryBuilder,
    selector: ScahStringView,
    save: ScahSave,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe { push_section(true, builder, selector, save, out_error) }
}

/// Nest copies of `children` under the builder's last section.
///
/// Each child query runs inside every match of that section, and its results
/// are read with `scah_element_get`. The children are copied, not consumed:
/// they remain owned by the caller and later changes to them do not affect
/// `builder`. A builder may be passed as its own child. `children` may be
/// NULL when `count` is zero.
///
/// # Safety
///
/// `builder` must be NULL or a live builder not in use elsewhere. `children`
/// must be NULL or point to `count` readable pointers, each NULL or a live
/// builder. `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_then(
    builder: *mut ScahQueryBuilder,
    children: *const *const ScahQueryBuilder,
    count: usize,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        if builder.is_null() {
            return Err(Failure::null("builder"));
        }
        let children: &[*const ScahQueryBuilder] = match count {
            0 => &[],
            _ if children.is_null() => return Err(Failure::null("children")),
            // SAFETY: the caller guarantees `count` readable pointers.
            _ => unsafe { std::slice::from_raw_parts(children, count) },
        };
        // Copy the children before borrowing `builder` mutably, since a
        // builder may be one of its own children.
        let children = children
            .iter()
            .enumerate()
            .map(|(index, &child)| {
                // SAFETY: the caller passes NULL or live builders.
                match unsafe { child.as_ref() } {
                    Some(child) => Ok(child.inner.clone()),
                    None => Err(Failure::null(&format!("children[{index}]"))),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        // SAFETY: checked non-NULL above; the caller guarantees it is live
        // and unaliased, and no child borrow outlives the copies.
        let builder = unsafe { &mut *builder };
        builder.inner.then_mut(|_| children);
        Ok(())
    })
}

/// Copy a builder, so a shared prefix can be extended in several ways.
///
/// # Safety
///
/// `builder` must be NULL or a live builder. `out_builder` and `out_error`
/// must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_clone(
    builder: *const ScahQueryBuilder,
    out_builder: *mut *mut ScahQueryBuilder,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_builder`.
        let out_builder = unsafe { out_handle(out_builder, "out_builder") }?;
        // SAFETY: the caller passes NULL or a live builder.
        let builder = unsafe { deref(builder, "builder") }?;
        let clone = Box::new(ScahQueryBuilder {
            inner: builder.inner.clone(),
        });
        // SAFETY: checked non-NULL and writable above.
        unsafe { out_builder.write(Box::into_raw(clone)) };
        Ok(())
    })
}

/// Free a builder. NULL is ignored.
///
/// # Safety
///
/// `builder` must be NULL or a builder from this library that has not been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_builder_free(builder: *mut ScahQueryBuilder) {
    guard_value((), || {
        if !builder.is_null() {
            // SAFETY: the caller transfers ownership of a live builder.
            drop(unsafe { Box::from_raw(builder) });
        }
    })
}

/// Compile a builder into a query.
///
/// The builder is left untouched and may be reused or freed. Fails with
/// `SCAH_STATUS_INVALID_SELECTOR` when any selector is invalid.
///
/// # Safety
///
/// `builder` must be NULL or a live builder. `out_query` and `out_error`
/// must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_build(
    builder: *const ScahQueryBuilder,
    out_query: *mut *mut ScahQuery,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_query`.
        let out_query = unsafe { out_handle(out_query, "out_query") }?;
        // SAFETY: the caller passes NULL or a live builder.
        let builder = unsafe { deref(builder, "builder") }?;
        let query = OwnedQuery::build(builder.inner.clone()).map_err(|err| {
            Failure::new(
                ScahStatus::InvalidSelector,
                format!("invalid selector: {err}"),
            )
        })?;
        let query = Box::new(ScahQuery { inner: query });
        // SAFETY: checked non-NULL and writable above.
        unsafe { out_query.write(Box::into_raw(query)) };
        Ok(())
    })
}

/// Free a query. NULL is ignored. Stores parsed with it stay valid.
///
/// # Safety
///
/// `query` must be NULL or a query from this library that has not been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_query_free(query: *mut ScahQuery) {
    guard_value((), || {
        if !query.is_null() {
            // SAFETY: the caller transfers ownership of a live query.
            drop(unsafe { Box::from_raw(query) });
        }
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::error::tests::take_message;
    use std::ptr;

    pub(crate) const ALL: ScahSave = ScahSave {
        inner_html: true,
        raw_text: true,
        text: true,
        attributes: true,
    };

    pub(crate) const NAME_ONLY: ScahSave = ScahSave {
        inner_html: false,
        raw_text: false,
        text: false,
        attributes: false,
    };

    pub(crate) fn builder(selector: &str, save: ScahSave) -> *mut ScahQueryBuilder {
        let mut builder = ptr::null_mut();
        // SAFETY: the view borrows a live string and the out-pointer is a local.
        let status = unsafe {
            scah_query_builder_new_all(
                ScahStringView::new(selector),
                save,
                &mut builder,
                ptr::null_mut(),
            )
        };
        assert_eq!(status, ScahStatus::Ok);
        builder
    }

    /// Compile and free `builder`.
    pub(crate) fn build(builder: *mut ScahQueryBuilder) -> *mut ScahQuery {
        let mut query = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: `builder` is live and owned by the test; outputs are locals.
        let status = unsafe { scah_query_build(builder, &mut query, &mut error) };
        if status != ScahStatus::Ok {
            panic!("{status:?}: {}", take_message(error));
        }
        // SAFETY: the test owns `builder`.
        unsafe { scah_query_builder_free(builder) };
        query
    }

    fn sections(builder: *const ScahQueryBuilder) -> usize {
        // SAFETY: tests only pass live builders.
        unsafe { &*builder }.inner.len()
    }

    #[test]
    fn save_converts_field_by_field() {
        assert_eq!(Save::from(ALL), Save::all());
        assert_eq!(Save::from(NAME_ONLY), Save::name_only());
    }

    #[test]
    fn invalid_selectors_are_reported_at_build() {
        let builder = builder("main", ALL);
        // SAFETY: `builder` is live and the selector view borrows a literal.
        let status = unsafe {
            scah_query_builder_all(builder, ScahStringView::new("a["), ALL, ptr::null_mut())
        };
        assert_eq!(status, ScahStatus::Ok);

        let mut query = ptr::dangling_mut();
        let mut error = ptr::null_mut();
        // SAFETY: `builder` is live; outputs are locals.
        let status = unsafe { scah_query_build(builder, &mut query, &mut error) };
        assert_eq!(status, ScahStatus::InvalidSelector);
        assert!(query.is_null());
        assert!(take_message(error).starts_with("invalid selector: "));
        // SAFETY: the test owns `builder`.
        unsafe { scah_query_builder_free(builder) };
    }

    #[test]
    fn then_copies_children_without_consuming_them() {
        let parent = builder("ul", ALL);
        let li = builder("li", ALL);
        let a = builder("a", ALL);
        // SAFETY: all builders are live.
        let status = unsafe {
            scah_query_builder_first(a, ScahStringView::new("span"), ALL, ptr::null_mut())
        };
        assert_eq!(status, ScahStatus::Ok);

        let children = [li.cast_const(), a.cast_const()];
        // SAFETY: all builders are live and `children` holds two pointers.
        let status =
            unsafe { scah_query_builder_then(parent, children.as_ptr(), 2, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        assert_eq!(sections(parent), 4);
        assert_eq!(sections(li), 1);
        assert_eq!(sections(a), 2);

        // A builder may nest a copy of itself.
        let this = [parent.cast_const()];
        // SAFETY: `parent` is live and `this` holds one pointer.
        let status = unsafe { scah_query_builder_then(parent, this.as_ptr(), 1, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        assert_eq!(sections(parent), 8);

        // SAFETY: no children with a NULL array is a no-op.
        let status = unsafe { scah_query_builder_then(parent, ptr::null(), 0, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);

        // SAFETY: the test owns all builders.
        unsafe {
            scah_query_builder_free(li);
            scah_query_builder_free(a);
        }
        // SAFETY: the test owns the built query.
        unsafe { scah_query_free(build(parent)) };
    }

    #[test]
    fn then_rejects_null_children() {
        let parent = builder("ul", ALL);
        let children = [ptr::null()];
        let mut error = ptr::null_mut();
        // SAFETY: `parent` is live and `children` holds one pointer.
        let status = unsafe { scah_query_builder_then(parent, children.as_ptr(), 1, &mut error) };
        assert_eq!(status, ScahStatus::NullPointer);
        assert_eq!(take_message(error), "`children[0]` must not be NULL");
        assert_eq!(sections(parent), 1);

        // SAFETY: a NULL array with a count is rejected before reading.
        let status = unsafe { scah_query_builder_then(parent, ptr::null(), 1, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::NullPointer);
        // SAFETY: the test owns `parent`.
        unsafe { scah_query_builder_free(parent) };
    }

    #[test]
    fn clone_is_independent() {
        let original = builder("ul", ALL);
        let mut clone = ptr::null_mut();
        // SAFETY: `original` is live; the out-pointer is a local.
        let status = unsafe { scah_query_builder_clone(original, &mut clone, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        // SAFETY: `clone` is live.
        let status = unsafe {
            scah_query_builder_all(clone, ScahStringView::new("li"), ALL, ptr::null_mut())
        };
        assert_eq!(status, ScahStatus::Ok);
        assert_eq!(sections(original), 1);
        assert_eq!(sections(clone), 2);
        // SAFETY: the test owns both builders.
        unsafe {
            scah_query_builder_free(original);
            scah_query_builder_free(clone);
        }
    }

    #[test]
    fn null_arguments_are_rejected() {
        let mut builder = ptr::dangling_mut();
        let mut error = ptr::null_mut();
        // SAFETY: NULL arguments are rejected before use.
        unsafe {
            let status = scah_query_builder_new_all(
                ScahStringView::new("a"),
                ALL,
                ptr::null_mut(),
                &mut error,
            );
            assert_eq!(status, ScahStatus::NullPointer);
            assert_eq!(take_message(error), "`out_builder` must not be NULL");

            let selector = ScahStringView {
                data: ptr::null(),
                len: 1,
            };
            let status = scah_query_builder_new_first(selector, ALL, &mut builder, &mut error);
            assert_eq!(status, ScahStatus::NullPointer);
            assert!(builder.is_null());
            take_message(error);

            let status = scah_query_builder_all(
                ptr::null_mut(),
                ScahStringView::new("a"),
                ALL,
                ptr::null_mut(),
            );
            assert_eq!(status, ScahStatus::NullPointer);

            let mut query = ptr::null_mut();
            let status = scah_query_build(ptr::null(), &mut query, ptr::null_mut());
            assert_eq!(status, ScahStatus::NullPointer);

            scah_query_builder_free(ptr::null_mut());
            scah_query_free(ptr::null_mut());
        }
    }

    #[test]
    fn invalid_utf8_selectors_are_rejected() {
        let bytes = [0xffu8];
        let selector = ScahStringView {
            data: bytes.as_ptr(),
            len: 1,
        };
        let mut builder = ptr::null_mut();
        // SAFETY: the view borrows a live local; the out-pointer is a local.
        let status =
            unsafe { scah_query_builder_new_all(selector, ALL, &mut builder, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::InvalidUtf8);
        assert!(builder.is_null());
    }
}
