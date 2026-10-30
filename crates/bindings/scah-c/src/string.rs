//! Borrowed UTF-8 string views.

use crate::error::{Failure, Result, ScahStatus};

/// A borrowed UTF-8 string: `len` bytes starting at `data`.
///
/// The bytes are not NUL-terminated. `data` may be NULL only when `len` is
/// zero. Views returned by this library are never NULL with a non-zero `len`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ScahStringView {
    pub data: *const u8,
    pub len: usize,
}

/// A string view that may be absent.
///
/// `is_some == false` means the value is missing; `value` is then empty.
/// `is_some == true` with `value.len == 0` means the value is an empty string.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ScahOptionalStringView {
    pub is_some: bool,
    pub value: ScahStringView,
}

impl ScahStringView {
    pub(crate) const EMPTY: Self = Self {
        data: std::ptr::null(),
        len: 0,
    };

    /// View `value`. The view is valid for as long as `value` is.
    pub(crate) fn new(value: &str) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }

    /// Validate a caller-provided view as UTF-8.
    ///
    /// A NULL `data` with zero `len` is the empty string.
    ///
    /// # Safety
    ///
    /// A non-NULL `data` must be valid for reading `len` bytes that stay
    /// unmodified for `'a`.
    pub(crate) unsafe fn to_str<'a>(self, argument: &str) -> Result<&'a str> {
        if self.data.is_null() {
            return match self.len {
                0 => Ok(""),
                _ => Err(Failure::new(
                    ScahStatus::NullPointer,
                    format!("`{argument}.data` is NULL but `{argument}.len` is not zero"),
                )),
            };
        }
        // SAFETY: guaranteed by the caller.
        let bytes = unsafe { std::slice::from_raw_parts(self.data, self.len) };
        std::str::from_utf8(bytes).map_err(|err| {
            Failure::new(
                ScahStatus::InvalidUtf8,
                format!("`{argument}` is not valid UTF-8: {err}"),
            )
        })
    }
}

impl From<Option<&str>> for ScahOptionalStringView {
    fn from(value: Option<&str>) -> Self {
        match value {
            Some(value) => Self {
                is_some: true,
                value: ScahStringView::new(value),
            },
            None => Self {
                is_some: false,
                value: ScahStringView::EMPTY,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(bytes: &[u8]) -> ScahStringView {
        ScahStringView {
            data: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    #[test]
    fn null_empty_view_is_the_empty_string() {
        // SAFETY: a NULL, empty view reads no memory.
        assert_eq!(unsafe { ScahStringView::EMPTY.to_str("s") }.unwrap(), "");
    }

    #[test]
    fn null_view_with_length_is_rejected() {
        let view = ScahStringView {
            data: std::ptr::null(),
            len: 3,
        };
        // SAFETY: a NULL view is rejected before any read.
        let err = unsafe { view.to_str("s") }.unwrap_err();
        assert_eq!(err.status, ScahStatus::NullPointer);
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        // SAFETY: the view borrows a live local array.
        let err = unsafe { view(&[b'a', 0xff]).to_str("s") }.unwrap_err();
        assert_eq!(err.status, ScahStatus::InvalidUtf8);
        assert!(err.message.starts_with("`s` is not valid UTF-8"));
    }

    #[test]
    fn views_round_trip() {
        // SAFETY: the view borrows a live string literal.
        assert_eq!(
            unsafe { ScahStringView::new("hé").to_str("s") }.unwrap(),
            "hé"
        );
    }

    #[test]
    fn optional_views_distinguish_missing_from_empty() {
        let missing = ScahOptionalStringView::from(None);
        assert!(!missing.is_some);
        assert!(missing.value.data.is_null());

        let empty = ScahOptionalStringView::from(Some(""));
        assert!(empty.is_some);
        assert_eq!(empty.value.len, 0);
        assert!(!empty.value.data.is_null());
    }
}
