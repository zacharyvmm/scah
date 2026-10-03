//! Parsed stores, element lists, and element getters.

use crate::error::{
    ErrorSlot, Failure, Result, ScahError, ScahStatus, deref, guard, guard_value, out, out_handle,
};
use crate::query::ScahQuery;
use crate::string::{ScahOptionalStringView, ScahStringView};
use scah::{Element, ParseError, Store};
use scah_ffi::OwnedStore;

/// Identifies an element within the store that produced it.
///
/// Ids index the store's element arena: they run from 0 to
/// `scah_store_len(store) - 1` and are only meaningful with that store.
pub type ScahElementId = usize;

/// The result of parsing HTML against one or more queries.
///
/// Created by `scah_parse` and released with `scah_store_free`. The store
/// owns a copy of the HTML and shares its queries' selector strings. Every
/// string view read from it stays valid until it is freed.
pub struct ScahStore {
    inner: OwnedStore,
}

/// The ids of the elements matched by one selector, in document order.
///
/// Created by `scah_store_get` or `scah_element_get` and released with
/// `scah_element_list_free`. A list owns its ids and may outlive its store,
/// though the ids are then meaningless.
pub struct ScahElementList {
    ids: Box<[ScahElementId]>,
}

/// Parse `html` against `queries`.
///
/// The store copies `html` and shares the selector strings of each query, so
/// both may be freed once this returns. On success `*out_store` receives a
/// store to free with `scah_store_free`.
///
/// # Safety
///
/// `html` must be a readable view. `queries` must be NULL or point to
/// `count` readable pointers, each NULL or a live query. `out_store` and
/// `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_parse(
    html: ScahStringView,
    queries: *const *const ScahQuery,
    count: usize,
    out_store: *mut *mut ScahStore,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_store`.
        let out_store = unsafe { out_handle(out_store, "out_store") }?;
        if count == 0 {
            return Err(ParseError::EmptyQueries.into());
        }
        if queries.is_null() {
            return Err(Failure::null("queries"));
        }
        // SAFETY: the caller guarantees `count` readable pointers.
        let queries = unsafe { std::slice::from_raw_parts(queries, count) };
        let queries = queries
            .iter()
            .enumerate()
            .map(|(index, &query)| {
                // SAFETY: the caller passes NULL or live queries.
                match unsafe { query.as_ref() } {
                    Some(query) => Ok(&query.inner),
                    None => Err(Failure::null(&format!("queries[{index}]"))),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        // SAFETY: the caller passes a readable view.
        let html = unsafe { html.to_str("html") }?.to_owned();

        let inner = OwnedStore::parse(html, &queries)?;
        let store = Box::new(ScahStore { inner });
        // SAFETY: checked non-NULL and writable above.
        unsafe { out_store.write(Box::into_raw(store)) };
        Ok(())
    })
}

impl From<ParseError> for Failure {
    fn from(err: ParseError) -> Self {
        let status = match err {
            ParseError::EmptyQueries => ScahStatus::EmptyQueries,
            ParseError::MaximumDepthExceeded => ScahStatus::MaximumDepthExceeded,
            // Only `parse_without_text_capture` reports this.
            ParseError::TextCaptureRequired => ScahStatus::InternalPanic,
        };
        Failure::new(status, err.to_string())
    }
}

/// Free a store. NULL is ignored.
///
/// Invalidates every string view read from the store.
///
/// # Safety
///
/// `store` must be NULL or a store from this library that has not been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_store_free(store: *mut ScahStore) {
    guard_value((), || {
        if !store.is_null() {
            // SAFETY: the caller transfers ownership of a live store.
            drop(unsafe { Box::from_raw(store) });
        }
    })
}

/// The number of elements in the store, across all queries. NULL yields 0.
///
/// # Safety
///
/// `store` must be NULL or a live store.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_store_len(store: *const ScahStore) -> usize {
    guard_value(0, || {
        // SAFETY: the caller passes NULL or a live store.
        unsafe { store.as_ref() }.map_or(0, |store| store.inner.store().elements.len())
    })
}

/// Look up the elements matched by a top-level query section.
///
/// `selector` must equal the selector string the section was built with. When
/// nothing matched it, or no query has such a section, this succeeds and sets
/// `*out_list` to NULL. Otherwise `*out_list` receives a non-empty list to
/// free with `scah_element_list_free`.
///
/// # Safety
///
/// `store` must be NULL or a live store. `selector` must be a readable view.
/// `out_list` and `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_store_get(
    store: *const ScahStore,
    selector: ScahStringView,
    out_list: *mut *mut ScahElementList,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_list`.
        let out_list = unsafe { out_handle(out_list, "out_list") }?;
        // SAFETY: the caller passes NULL or a live store.
        let store = unsafe { deref(store, "store") }?.inner.store();
        // SAFETY: the caller passes a readable view.
        let selector = unsafe { selector.to_str("selector") }?;
        if let Some(elements) = store.get(selector) {
            // SAFETY: checked non-NULL and writable above.
            unsafe { out_list.write(new_list(store, elements)) };
        }
        Ok(())
    })
}

/// Look up the elements matched by a child query nested under `element`
/// with `scah_query_builder_then`.
///
/// `selector` must equal the child's selector string. Like `scah_store_get`,
/// this succeeds with `*out_list` set to NULL when there are no matches.
///
/// # Safety
///
/// Same as `scah_store_get`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_get(
    store: *const ScahStore,
    element: ScahElementId,
    selector: ScahStringView,
    out_list: *mut *mut ScahElementList,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_list`.
        let out_list = unsafe { out_handle(out_list, "out_list") }?;
        // SAFETY: the caller passes NULL or a live store.
        let (store, element) = unsafe { lookup(store, element) }?;
        // SAFETY: the caller passes a readable view.
        let selector = unsafe { selector.to_str("selector") }?;
        if let Some(elements) = element.get(store, selector) {
            // SAFETY: checked non-NULL and writable above.
            unsafe { out_list.write(new_list(store, elements)) };
        }
        Ok(())
    })
}

/// Collect `elements` into a list, or NULL when there are none.
fn new_list<'a>(
    store: &'a Store<'a, 'a>,
    elements: impl Iterator<Item = &'a Element<'a>>,
) -> *mut ScahElementList {
    let ids: Box<[_]> = elements
        // SAFETY: the iterator yields elements from this store's arena.
        .map(|element| unsafe { store.elements.index_of(element) }.index())
        .collect();
    if ids.is_empty() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(ScahElementList { ids }))
}

/// The number of ids in `list`. NULL yields 0, so a missing result can be
/// treated as an empty list.
///
/// # Safety
///
/// `list` must be NULL or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_list_len(list: *const ScahElementList) -> usize {
    guard_value(0, || {
        // SAFETY: the caller passes NULL or a live list.
        unsafe { list.as_ref() }.map_or(0, |list| list.ids.len())
    })
}

/// The ids in `list`, valid until `scah_element_list_free(list)`. NULL
/// yields NULL.
///
/// # Safety
///
/// `list` must be NULL or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_list_ids(
    list: *const ScahElementList,
) -> *const ScahElementId {
    guard_value(std::ptr::null(), || {
        // SAFETY: the caller passes NULL or a live list.
        unsafe { list.as_ref() }.map_or(std::ptr::null(), |list| list.ids.as_ptr())
    })
}

/// Free a list. NULL is ignored.
///
/// # Safety
///
/// `list` must be NULL or a list from this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_list_free(list: *mut ScahElementList) {
    guard_value((), || {
        if !list.is_null() {
            // SAFETY: the caller transfers ownership of a live list.
            drop(unsafe { Box::from_raw(list) });
        }
    })
}

/// Borrow element `element` of a caller's store.
///
/// # Safety
///
/// `store` must be NULL or a live store that outlives `'a`.
unsafe fn lookup<'a>(
    store: *const ScahStore,
    element: ScahElementId,
) -> Result<(&'a Store<'a, 'a>, &'a Element<'a>)> {
    // SAFETY: guaranteed by the caller.
    let store = unsafe { deref(store, "store") }?.inner.store();
    let element = store.elements.get(element).ok_or_else(|| {
        Failure::new(
            ScahStatus::IndexOutOfBounds,
            format!(
                "element id {element} is out of bounds for a store of {} elements",
                store.elements.len()
            ),
        )
    })?;
    Ok((store, element))
}

/// Read one value from an element into `*out_value`.
///
/// # Safety
///
/// `store` must be NULL or a live store. `out_value` and `out_error` must be
/// NULL or writable.
unsafe fn read<T>(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut T,
    out_error: *mut *mut ScahError,
    get: impl for<'a> FnOnce(&'a Store<'a, 'a>, &'a Element<'a>) -> T,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        let out_value = out(out_value, "out_value")?;
        // SAFETY: the caller passes NULL or a live store.
        let (store, element) = unsafe { lookup(store, element) }?;
        // SAFETY: checked non-NULL above; the caller guarantees writable.
        unsafe { out_value.write(get(store, element)) };
        Ok(())
    })
}

/// The element's tag name.
///
/// # Safety
///
/// `store` must be NULL or a live store. `out_value` and `out_error` must be
/// NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_name(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |_, element| {
            ScahStringView::new(element.name)
        })
    }
}

/// The element's `id` attribute. Missing when absent or when the query did
/// not save attributes.
///
/// # Safety
///
/// Same as `scah_element_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_id(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |_, element| {
            element.id.into()
        })
    }
}

/// The element's `class` attribute. Missing when absent or when the query
/// did not save attributes.
///
/// # Safety
///
/// Same as `scah_element_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_class_name(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |_, element| {
            element.class.into()
        })
    }
}

/// The HTML between the element's tags. Missing unless the query saved
/// `inner_html`.
///
/// # Safety
///
/// Same as `scah_element_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_inner_html(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |_, element| {
            element.inner_html.into()
        })
    }
}

/// The element's descendant text as written in the source. Missing unless
/// the query saved `raw_text`.
///
/// # Safety
///
/// Same as `scah_element_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_raw_text(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |store, element| {
            element.raw_text(store).into()
        })
    }
}

/// The element's descendant text with entities decoded and whitespace
/// collapsed. Missing unless the query saved `text`.
///
/// # Safety
///
/// Same as `scah_element_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_text(
    store: *const ScahStore,
    element: ScahElementId,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_value, out_error, |store, element| {
            element.text(store).into()
        })
    }
}

/// The value of the attribute named `key`, compared ASCII
/// case-insensitively.
///
/// Missing when the attribute is absent, has no value (as in `<input
/// disabled>`), or the query did not save attributes. Present but empty for
/// `key=""`. Use `scah_element_attribute_at` to tell an absent attribute from
/// one without a value.
///
/// # Safety
///
/// `store` must be NULL or a live store. `key` must be a readable view.
/// `out_value` and `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_attribute(
    store: *const ScahStore,
    element: ScahElementId,
    key: ScahStringView,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        let out_value = out(out_value, "out_value")?;
        // SAFETY: the caller passes NULL or a live store.
        let (store, element) = unsafe { lookup(store, element) }?;
        // SAFETY: the caller passes a readable view.
        let key = unsafe { key.to_str("key") }?;
        let value = element.attribute(store, key).into();
        // SAFETY: checked non-NULL above; the caller guarantees writable.
        unsafe { out_value.write(value) };
        Ok(())
    })
}

/// The number of attributes saved for the element, other than `id` and
/// `class`.
///
/// # Safety
///
/// `store` must be NULL or a live store. `out_count` and `out_error` must be
/// NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_attribute_count(
    store: *const ScahStore,
    element: ScahElementId,
    out_count: *mut usize,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: forwarded caller guarantees.
    unsafe {
        read(store, element, out_count, out_error, |store, element| {
            element.attributes(store).map_or(0, <[_]>::len)
        })
    }
}

/// The attribute at `index`, in source order, where `index` is below
/// `scah_element_attribute_count`.
///
/// `*out_key` receives the name as written in the source. `*out_value` is
/// missing for an attribute without a value (as in `<input disabled>`).
///
/// # Safety
///
/// `store` must be NULL or a live store. `out_key`, `out_value`, and
/// `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_element_attribute_at(
    store: *const ScahStore,
    element: ScahElementId,
    index: usize,
    out_key: *mut ScahStringView,
    out_value: *mut ScahOptionalStringView,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        let out_key = out(out_key, "out_key")?;
        let out_value = out(out_value, "out_value")?;
        // SAFETY: the caller passes NULL or a live store.
        let (store, element) = unsafe { lookup(store, element) }?;
        let attributes = element.attributes(store).unwrap_or_default();
        let attribute = attributes.get(index).ok_or_else(|| {
            Failure::new(
                ScahStatus::IndexOutOfBounds,
                format!(
                    "attribute index {index} is out of bounds for an element with {} attributes",
                    attributes.len()
                ),
            )
        })?;
        // SAFETY: both checked non-NULL above; the caller guarantees writable.
        unsafe {
            out_key.write(ScahStringView::new(attribute.key));
            out_value.write(attribute.value.into());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::tests::take_message;
    use crate::query::tests::{ALL, NAME_ONLY, build, builder};
    use crate::query::{scah_query_builder_free, scah_query_builder_then, scah_query_free};
    use std::ptr;

    fn view(value: &str) -> ScahStringView {
        ScahStringView::new(value)
    }

    fn string(view: ScahStringView) -> String {
        // SAFETY: tests only read views from live stores.
        unsafe { view.to_str("view") }.unwrap().to_owned()
    }

    fn optional(view: ScahOptionalStringView) -> Option<String> {
        view.is_some.then(|| string(view.value))
    }

    /// Parse `html` against `queries`, freeing the queries.
    fn parse(html: &str, queries: &[*mut ScahQuery]) -> *mut ScahStore {
        let html = html.to_owned();
        let mut store = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: the pointers are live queries and the view borrows `html`.
        let status = unsafe {
            scah_parse(
                view(&html),
                queries.as_ptr().cast(),
                queries.len(),
                &mut store,
                &mut error,
            )
        };
        if status != ScahStatus::Ok {
            panic!("{status:?}: {}", take_message(error));
        }
        drop(html);
        for &query in queries {
            // SAFETY: the test owns every query.
            unsafe { scah_query_free(query) };
        }
        store
    }

    fn ids(list: *mut ScahElementList) -> Vec<ScahElementId> {
        // SAFETY: the list is NULL or live, and freed after copying.
        unsafe {
            let len = scah_element_list_len(list);
            let ids = match len {
                0 => Vec::new(),
                _ => std::slice::from_raw_parts(scah_element_list_ids(list), len).to_vec(),
            };
            scah_element_list_free(list);
            ids
        }
    }

    fn get(store: *const ScahStore, selector: &str) -> Option<Vec<ScahElementId>> {
        let mut list = ptr::dangling_mut();
        // SAFETY: `store` is live and the out-pointer is a local.
        let status = unsafe { scah_store_get(store, view(selector), &mut list, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        (!list.is_null()).then(|| ids(list))
    }

    fn nested(
        store: *const ScahStore,
        element: ScahElementId,
        selector: &str,
    ) -> Option<Vec<ScahElementId>> {
        let mut list = ptr::dangling_mut();
        // SAFETY: `store` is live and the out-pointer is a local.
        let status =
            unsafe { scah_element_get(store, element, view(selector), &mut list, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        (!list.is_null()).then(|| ids(list))
    }

    type Getter = unsafe extern "C" fn(
        *const ScahStore,
        ScahElementId,
        *mut ScahOptionalStringView,
        *mut *mut ScahError,
    ) -> ScahStatus;

    fn field(store: *const ScahStore, element: ScahElementId, getter: Getter) -> Option<String> {
        let mut value = ScahOptionalStringView::from(None);
        // SAFETY: `store` is live and the out-pointer is a local.
        let status = unsafe { getter(store, element, &mut value, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        optional(value)
    }

    fn name(store: *const ScahStore, element: ScahElementId) -> String {
        let mut value = ScahStringView::EMPTY;
        // SAFETY: `store` is live and the out-pointer is a local.
        let status = unsafe { scah_element_name(store, element, &mut value, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        string(value)
    }

    fn attribute(store: *const ScahStore, element: ScahElementId, key: &str) -> Option<String> {
        let mut value = ScahOptionalStringView::from(None);
        // SAFETY: `store` is live and the out-pointer is a local.
        let status = unsafe {
            scah_element_attribute(store, element, view(key), &mut value, ptr::null_mut())
        };
        assert_eq!(status, ScahStatus::Ok);
        optional(value)
    }

    fn attributes(
        store: *const ScahStore,
        element: ScahElementId,
    ) -> Vec<(String, Option<String>)> {
        let mut count = usize::MAX;
        // SAFETY: `store` is live and the out-pointers are locals.
        unsafe {
            let status = scah_element_attribute_count(store, element, &mut count, ptr::null_mut());
            assert_eq!(status, ScahStatus::Ok);
            (0..count)
                .map(|index| {
                    let mut key = ScahStringView::EMPTY;
                    let mut value = ScahOptionalStringView::from(None);
                    let status = scah_element_attribute_at(
                        store,
                        element,
                        index,
                        &mut key,
                        &mut value,
                        ptr::null_mut(),
                    );
                    assert_eq!(status, ScahStatus::Ok);
                    (string(key), optional(value))
                })
                .collect()
        }
    }

    const LISTING: &str = concat!(
        "<ul id='menu' class='nav'>",
        "<li><a href='/a' data-x=''>A &amp; <b>b</b></a></li>",
        "<li><a href='/b' disabled>B</a></li>",
        "</ul>",
        "<p>  spaced   out  </p>",
    );

    #[test]
    fn reads_nested_results_after_freeing_queries_and_html() {
        let ul = builder("ul", ALL);
        let li = builder("li", ALL);
        let a = builder("a", ALL);
        // SAFETY: all builders are live.
        unsafe {
            let children = [a.cast_const()];
            let status = scah_query_builder_then(li, children.as_ptr(), 1, ptr::null_mut());
            assert_eq!(status, ScahStatus::Ok);
            let children = [li.cast_const()];
            let status = scah_query_builder_then(ul, children.as_ptr(), 1, ptr::null_mut());
            assert_eq!(status, ScahStatus::Ok);
            scah_query_builder_free(li);
            scah_query_builder_free(a);
        }
        let store = parse(LISTING, &[build(ul), build(builder("p", ALL))]);

        let [ul] = get(store, "ul").unwrap()[..] else {
            panic!("expected one ul");
        };
        assert_eq!(name(store, ul), "ul");
        assert_eq!(field(store, ul, scah_element_id).as_deref(), Some("menu"));
        assert_eq!(
            field(store, ul, scah_element_class_name).as_deref(),
            Some("nav")
        );
        assert_eq!(get(store, "li"), None, "nested sections are not top-level");
        assert_eq!(attributes(store, ul), []);

        let items = nested(store, ul, "li").unwrap();
        assert_eq!(items.len(), 2);
        let links: Vec<_> = items
            .iter()
            .map(|&li| nested(store, li, "a").unwrap()[0])
            .collect();
        assert_eq!(
            field(store, links[0], scah_element_text).as_deref(),
            Some("A & b")
        );
        assert_eq!(
            field(store, links[0], scah_element_raw_text).as_deref(),
            Some("A &amp; b")
        );
        assert_eq!(
            field(store, links[0], scah_element_inner_html).as_deref(),
            Some("A &amp; <b>b</b>")
        );
        assert_eq!(field(store, links[0], scah_element_id), None);

        assert_eq!(attribute(store, links[0], "HREF").as_deref(), Some("/a"));
        assert_eq!(attribute(store, links[0], "data-x").as_deref(), Some(""));
        assert_eq!(attribute(store, links[0], "missing"), None);
        assert_eq!(attribute(store, links[1], "disabled"), None);
        assert_eq!(
            attributes(store, links[1]),
            [
                ("href".to_owned(), Some("/b".to_owned())),
                ("disabled".to_owned(), None)
            ]
        );

        let [p] = get(store, "p").unwrap()[..] else {
            panic!("expected one p");
        };
        assert_eq!(
            field(store, p, scah_element_text).as_deref(),
            Some("spaced out")
        );
        assert_eq!(nested(store, p, "a"), None);

        // SAFETY: the test owns `store`.
        assert_eq!(unsafe { scah_store_len(store) }, 6);
        // SAFETY: the test owns `store`.
        unsafe { scah_store_free(store) };
    }

    #[test]
    fn unsaved_content_is_missing() {
        let store = parse(
            "<a id='x' href='/'>hi</a>",
            &[build(builder("a", NAME_ONLY))],
        );
        let a = get(store, "a").unwrap()[0];
        assert_eq!(name(store, a), "a");
        assert_eq!(field(store, a, scah_element_id), None);
        assert_eq!(field(store, a, scah_element_text), None);
        assert_eq!(field(store, a, scah_element_raw_text), None);
        assert_eq!(field(store, a, scah_element_inner_html), None);
        assert_eq!(attribute(store, a, "href"), None);
        assert!(attributes(store, a).is_empty());
        // SAFETY: the test owns `store`.
        unsafe { scah_store_free(store) };
    }

    #[test]
    fn selectors_without_matches_yield_no_list() {
        let ul = builder("ul", ALL);
        let li = builder("li", ALL);
        let children = [li.cast_const()];
        // SAFETY: both builders are live and owned by the test.
        unsafe {
            let status = scah_query_builder_then(ul, children.as_ptr(), 1, ptr::null_mut());
            assert_eq!(status, ScahStatus::Ok);
            scah_query_builder_free(li);
        }
        let store = parse(
            "<ul></ul><ul><li></li></ul>",
            &[build(ul), build(builder("b", ALL))],
        );
        assert_eq!(get(store, "b"), None);
        assert_eq!(get(store, "never-queried"), None);

        let lists = get(store, "ul").unwrap();
        assert_eq!(lists.len(), 2);
        assert_eq!(nested(store, lists[0], "li"), None);
        assert_eq!(nested(store, lists[1], "li").map(|li| li.len()), Some(1));
        assert_eq!(nested(store, lists[1], "span"), None);
        // SAFETY: the test owns `store`.
        unsafe { scah_store_free(store) };
    }

    #[test]
    fn out_of_range_ids_and_indexes_are_rejected() {
        let store = parse("<a href='/'></a>", &[build(builder("a", ALL))]);
        let mut value = ScahOptionalStringView::from(None);
        let mut key = ScahStringView::EMPTY;
        let mut list = ptr::dangling_mut();
        let mut error = ptr::null_mut();
        // SAFETY: `store` is live and the out-pointers are locals.
        unsafe {
            let status = scah_element_text(store, 1, &mut value, &mut error);
            assert_eq!(status, ScahStatus::IndexOutOfBounds);
            assert_eq!(
                take_message(error),
                "element id 1 is out of bounds for a store of 1 elements"
            );

            let status = scah_element_get(store, 7, view("a"), &mut list, ptr::null_mut());
            assert_eq!(status, ScahStatus::IndexOutOfBounds);
            assert!(list.is_null());

            let status = scah_element_attribute_at(store, 0, 1, &mut key, &mut value, &mut error);
            assert_eq!(status, ScahStatus::IndexOutOfBounds);
            take_message(error);

            scah_store_free(store);
        }
    }

    #[test]
    fn parse_errors_are_reported() {
        let mut store = ptr::dangling_mut();
        let mut error = ptr::null_mut();
        let query = build(builder("a", ALL));
        let queries = [query.cast_const()];
        // SAFETY: the views and pointers borrow live locals.
        unsafe {
            let status = scah_parse(view("<a>"), ptr::null(), 0, &mut store, &mut error);
            assert_eq!(status, ScahStatus::EmptyQueries);
            assert!(store.is_null());
            assert_eq!(take_message(error), "parse requires at least one query");

            let html = [b'<', 0xff];
            let html = ScahStringView {
                data: html.as_ptr(),
                len: html.len(),
            };
            let status = scah_parse(html, queries.as_ptr(), 1, &mut store, ptr::null_mut());
            assert_eq!(status, ScahStatus::InvalidUtf8);

            let status = scah_parse(view(""), ptr::null(), 1, &mut store, ptr::null_mut());
            assert_eq!(status, ScahStatus::NullPointer);

            let nulls = [ptr::null()];
            let status = scah_parse(view(""), nulls.as_ptr(), 1, &mut store, &mut error);
            assert_eq!(status, ScahStatus::NullPointer);
            assert_eq!(take_message(error), "`queries[0]` must not be NULL");

            let status = scah_parse(
                ScahStringView::EMPTY,
                queries.as_ptr(),
                1,
                &mut store,
                ptr::null_mut(),
            );
            assert_eq!(status, ScahStatus::Ok);
            assert_eq!(scah_store_len(store), 0);
            scah_store_free(store);
            scah_query_free(query);
        }
    }

    #[test]
    fn parse_errors_map_to_statuses() {
        // Parsing HTML deep enough to overflow takes seconds in debug builds,
        // so check the mapping directly.
        let failure = Failure::from(ParseError::MaximumDepthExceeded);
        assert_eq!(failure.status, ScahStatus::MaximumDepthExceeded);
        assert_eq!(
            failure.message,
            ParseError::MaximumDepthExceeded.to_string()
        );
        let failure = Failure::from(ParseError::EmptyQueries);
        assert_eq!(failure.status, ScahStatus::EmptyQueries);
    }

    #[test]
    fn null_arguments_are_rejected() {
        let mut list = ptr::null_mut();
        let mut value = ScahOptionalStringView::from(None);
        let mut error = ptr::null_mut();
        // SAFETY: NULL arguments are rejected before use.
        unsafe {
            let status = scah_store_get(ptr::null(), view("a"), &mut list, &mut error);
            assert_eq!(status, ScahStatus::NullPointer);
            assert_eq!(take_message(error), "`store` must not be NULL");

            let status = scah_element_text(ptr::null(), 0, &mut value, ptr::null_mut());
            assert_eq!(status, ScahStatus::NullPointer);

            assert_eq!(scah_store_len(ptr::null()), 0);
            assert_eq!(scah_element_list_len(ptr::null()), 0);
            assert!(scah_element_list_ids(ptr::null()).is_null());
            scah_store_free(ptr::null_mut());
            scah_element_list_free(ptr::null_mut());
        }

        let store = parse("<a></a>", &[build(builder("a", ALL))]);
        // SAFETY: `store` is live; NULL out-pointers are rejected before use.
        unsafe {
            let status = scah_store_get(store, view("a"), ptr::null_mut(), ptr::null_mut());
            assert_eq!(status, ScahStatus::NullPointer);
            let status = scah_element_text(store, 0, ptr::null_mut(), &mut error);
            assert_eq!(status, ScahStatus::NullPointer);
            assert_eq!(take_message(error), "`out_value` must not be NULL");
            let mut value = ScahOptionalStringView::from(None);
            let status = scah_element_attribute_at(
                store,
                0,
                0,
                ptr::null_mut(),
                &mut value,
                ptr::null_mut(),
            );
            assert_eq!(status, ScahStatus::NullPointer);
            scah_store_free(store);
        }
    }

    #[test]
    fn handles_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ScahStore>();
        assert_send_sync::<ScahElementList>();
    }
}
