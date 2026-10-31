//! HTML built from UTF-16 pieces, for callers that cannot pass a whole
//! UTF-16 string at once.

use crate::error::{
    ErrorSlot, Failure, Result, ScahError, ScahStatus, guard, guard_value, out_handle,
};
use scah_ffi::utf16::{LoneSurrogateError, LoneSurrogates, Utf16Encoder};

/// HTML being encoded as UTF-8 from UTF-16 pieces.
///
/// Created by `scah_html_new`, extended with `scah_html_push_utf16`, and
/// either parsed, and freed, by `scah_parse_html` or released with
/// `scah_html_free`. A surrogate pair may be split between two pieces.
pub struct ScahHtml {
    pub(crate) encoder: Utf16Encoder,
}

/// Start empty HTML with room for `capacity` bytes of UTF-8, which is exact
/// for that many ASCII units.
///
/// A lone surrogate fails with `SCAH_STATUS_INVALID_UTF16` when
/// `reject_lone_surrogates` is true, and is otherwise replaced with U+FFFD.
/// On success `*out_html` receives HTML to pass to `scah_parse_html` or free
/// with `scah_html_free`.
///
/// # Safety
///
/// `out_html` and `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_html_new(
    capacity: usize,
    reject_lone_surrogates: bool,
    out_html: *mut *mut ScahHtml,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes a NULL or writable `out_html`.
        let out_html = unsafe { out_handle(out_html, "out_html") }?;
        // The parser rejects 4 GiB or more, so reserving more is pointless.
        let capacity = capacity.min(u32::MAX as usize);
        let encoder =
            Utf16Encoder::with_capacity(capacity, lone_surrogates(reject_lone_surrogates));
        let html = Box::new(ScahHtml { encoder });
        // SAFETY: checked non-NULL and writable above.
        unsafe { out_html.write(Box::into_raw(html)) };
        Ok(())
    })
}

/// Append `len` UTF-16 code units to `html`.
///
/// On failure `html` is left as it was before the call.
///
/// # Safety
///
/// `html` must be NULL or live HTML. `units` must be NULL with `len` 0, or
/// point to `len` readable units. `out_error` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_html_push_utf16(
    html: *mut ScahHtml,
    units: *const u16,
    len: usize,
    out_error: *mut *mut ScahError,
) -> ScahStatus {
    // SAFETY: the caller passes a NULL or writable `out_error`.
    let out_error = unsafe { ErrorSlot::new(out_error) };
    guard(out_error, || {
        // SAFETY: the caller passes NULL or live HTML that no other call uses.
        let html = unsafe { html.as_mut() }.ok_or_else(|| Failure::null("html"))?;
        // Every unit encodes to at least one byte.
        check_input_size(html.encoder.len().saturating_add(len))?;
        // SAFETY: the caller passes `len` readable units.
        let units = unsafe { utf16_slice(units, len) }?;
        html.encoder.push(units)?;
        Ok(())
    })
}

/// Free HTML that was not parsed. NULL is ignored.
///
/// # Safety
///
/// `html` must be NULL or live HTML, which this call takes ownership of.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_html_free(html: *mut ScahHtml) {
    guard_value((), || {
        if !html.is_null() {
            // SAFETY: the caller transfers ownership of live HTML.
            drop(unsafe { Box::from_raw(html) });
        }
    })
}

pub(crate) fn lone_surrogates(reject: bool) -> LoneSurrogates {
    if reject {
        LoneSurrogates::Reject
    } else {
        LoneSurrogates::Replace
    }
}

/// Fail for HTML of `len` bytes or more that the parser would reject anyway.
pub(crate) fn check_input_size(len: usize) -> Result<()> {
    if len >= u32::MAX as usize {
        return Err(Failure::new(
            ScahStatus::InputTooLarge,
            "the HTML is 4 GiB or longer",
        ));
    }
    Ok(())
}

/// # Safety
///
/// `units` must be NULL with `len` 0, or point to `len` readable units.
pub(crate) unsafe fn utf16_slice<'a>(units: *const u16, len: usize) -> Result<&'a [u16]> {
    if len == 0 {
        return Ok(&[]);
    }
    if units.is_null() {
        return Err(Failure::null("html"));
    }
    // SAFETY: guaranteed by the caller.
    Ok(unsafe { std::slice::from_raw_parts(units, len) })
}

impl From<LoneSurrogateError> for Failure {
    fn from(err: LoneSurrogateError) -> Self {
        Failure::new(ScahStatus::InvalidUtf16, format!("`html` has a {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::tests::take_message;
    use crate::query::tests::{ALL, build, builder};
    use crate::query::{ScahQuery, scah_query_free};
    use crate::store::tests::texts;
    use crate::store::{ScahStore, scah_parse_html, scah_parse_utf16, scah_store_free};
    use std::ptr;

    const LONE: [u16; 8] = [0x3C, 0x61, 0x3E, 0xD800, 0x3C, 0x2F, 0x61, 0x3E]; // <a>\u{D800}</a>

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn new_html(reject: bool) -> *mut ScahHtml {
        let mut html = ptr::null_mut();
        // SAFETY: the out-pointer is a local.
        let status = unsafe { scah_html_new(4, reject, &mut html, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        html
    }

    fn push(html: *mut ScahHtml, units: &[u16]) -> std::result::Result<(), (ScahStatus, String)> {
        let mut error = ptr::null_mut();
        // SAFETY: `html` is NULL or live and `units` is readable.
        match unsafe { scah_html_push_utf16(html, units.as_ptr(), units.len(), &mut error) } {
            ScahStatus::Ok => Ok(()),
            status => Err((status, take_message(error))),
        }
    }

    /// Run `parse` with a query for `a` and the store's out-pointer, and
    /// return the text of each `a`, or the failure.
    fn parse_a(
        parse: impl FnOnce(
            *const *const ScahQuery,
            *mut *mut ScahStore,
            *mut *mut ScahError,
        ) -> ScahStatus,
    ) -> std::result::Result<Vec<String>, (ScahStatus, String)> {
        let query = build(builder("a", ALL)).cast_const();
        let (mut store, mut error) = (ptr::null_mut(), ptr::null_mut());
        let status = parse(&query, &mut store, &mut error);
        // SAFETY: the test owns the query.
        unsafe { scah_query_free(query.cast_mut()) };
        if status != ScahStatus::Ok {
            assert!(store.is_null());
            return Err((status, take_message(error)));
        }
        let texts = texts(store, "a");
        // SAFETY: the test owns the store.
        unsafe { scah_store_free(store) };
        Ok(texts)
    }

    fn parse_utf16(
        units: &[u16],
        reject: bool,
    ) -> std::result::Result<Vec<String>, (ScahStatus, String)> {
        // SAFETY: `units` is readable and the rest come from `parse_a`.
        parse_a(|query, store, error| unsafe {
            scah_parse_utf16(units.as_ptr(), units.len(), reject, query, 1, store, error)
        })
    }

    fn parse_html(html: *mut ScahHtml) -> std::result::Result<Vec<String>, (ScahStatus, String)> {
        // SAFETY: the caller passes live HTML, which this call frees.
        parse_a(|query, store, error| unsafe { scah_parse_html(html, query, 1, store, error) })
    }

    #[test]
    fn utf16_parses_like_utf8() {
        let units = utf16("<a>x😀</a><a>é &amp; 漢</a>");
        assert_eq!(parse_utf16(&units, true).unwrap(), ["x😀", "é & 漢"]);

        // Split inside the surrogate pair.
        let html = new_html(true);
        push(html, &units[..5]).unwrap();
        push(html, &units[5..]).unwrap();
        assert_eq!(parse_html(html).unwrap(), ["x😀", "é & 漢"]);
    }

    #[test]
    fn lone_surrogates_are_replaced_or_rejected() {
        assert_eq!(parse_utf16(&LONE, false).unwrap(), ["\u{FFFD}"]);
        let lone = (
            ScahStatus::InvalidUtf16,
            "`html` has a lone surrogate U+D800 at UTF-16 offset 3".to_owned(),
        );
        assert_eq!(parse_utf16(&LONE, true).unwrap_err(), lone);

        // A high surrogate that ends the last piece is lone too.
        let html = new_html(true);
        push(html, &LONE[..4]).unwrap();
        assert_eq!(parse_html(html).unwrap_err(), lone);
    }

    #[test]
    fn a_failed_push_keeps_the_html() {
        let html = new_html(true);
        push(html, &utf16("<a>x")).unwrap();
        let failure = push(html, &[0x79, 0xDC00]).unwrap_err();
        assert_eq!(failure.0, ScahStatus::InvalidUtf16);
        assert_eq!(
            failure.1,
            "`html` has a lone surrogate U+DC00 at UTF-16 offset 5"
        );
        push(html, &utf16("</a>")).unwrap();
        assert_eq!(parse_html(html).unwrap(), ["x"]);
    }

    #[test]
    fn parse_html_frees_the_html_when_it_fails() {
        // Miri and leak checkers report the HTML if a failure leaks it.
        let html = new_html(false);
        let mut store = ptr::null_mut();
        // SAFETY: `html` is live; no queries are read.
        let status = unsafe { scah_parse_html(html, ptr::null(), 0, &mut store, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::EmptyQueries);

        let failure = parse_html(ptr::null_mut()).unwrap_err();
        assert_eq!(
            failure,
            (
                ScahStatus::NullPointer,
                "`html` must not be NULL".to_owned()
            )
        );
    }

    #[test]
    fn null_arguments_are_reported() {
        let failure = push(ptr::null_mut(), &[0x41]).unwrap_err();
        assert_eq!(
            failure,
            (
                ScahStatus::NullPointer,
                "`html` must not be NULL".to_owned()
            )
        );
        let html = new_html(false);
        // SAFETY: a NULL pointer with a length is reported, not read.
        let status = unsafe { scah_html_push_utf16(html, ptr::null(), 1, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::NullPointer);
        // SAFETY: empty input may be NULL.
        let status = unsafe { scah_html_push_utf16(html, ptr::null(), 0, ptr::null_mut()) };
        assert_eq!(status, ScahStatus::Ok);
        assert_eq!(parse_utf16(&[], true).unwrap(), Vec::<String>::new());
        // SAFETY: the test owns `html`; NULL is ignored.
        unsafe {
            scah_html_free(html);
            scah_html_free(ptr::null_mut());
        }
    }
}
