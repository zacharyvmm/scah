//! Status codes, error handles, and the panic guard around every entry point.

use crate::string::ScahStringView;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

/// Outcome of a fallible call.
///
/// Any status other than `SCAH_STATUS_OK` comes with a message in
/// `*out_error` when the caller passed a non-NULL `out_error`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScahStatus {
    /// The call succeeded.
    Ok = 0,
    /// A required pointer argument was NULL, or a string view had a NULL
    /// `data` pointer with a non-zero `len`.
    NullPointer = 1,
    /// An input string view was not valid UTF-8.
    InvalidUtf8 = 2,
    /// A selector could not be compiled.
    InvalidSelector = 3,
    /// `scah_parse` was called without any queries.
    EmptyQueries = 4,
    /// The HTML nests elements deeper than the parser supports.
    MaximumDepthExceeded = 5,
    /// An element id or attribute index was out of range.
    IndexOutOfBounds = 6,
    /// scah panicked. The library state is intact, but this is a bug worth
    /// reporting.
    InternalPanic = 7,
    /// An argument was invalid, such as an Arrow column requested twice.
    InvalidArgument = 8,
    /// A result was too large to export, such as HTML over 2 GiB for Arrow's
    /// 32-bit offsets.
    TooLarge = 9,
}

/// An error message returned through `out_error`.
///
/// Read it with `scah_error_message` and release it with `scah_error_free`.
pub struct ScahError {
    message: String,
}

/// A failed call: the status to return and the message to report.
#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) status: ScahStatus,
    pub(crate) message: String,
}

impl Failure {
    pub(crate) fn new(status: ScahStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub(crate) fn null(argument: &str) -> Self {
        Self::new(
            ScahStatus::NullPointer,
            format!("`{argument}` must not be NULL"),
        )
    }
}

pub(crate) type Result<T> = std::result::Result<T, Failure>;

/// A caller's `out_error` argument, cleared on creation.
pub(crate) struct ErrorSlot(*mut *mut ScahError);

impl ErrorSlot {
    /// # Safety
    ///
    /// `out_error` must be NULL or valid for writing one pointer for as long
    /// as the slot lives.
    pub(crate) unsafe fn new(out_error: *mut *mut ScahError) -> Self {
        if !out_error.is_null() {
            // SAFETY: guaranteed by the caller.
            unsafe { out_error.write(ptr::null_mut()) };
        }
        Self(out_error)
    }

    fn report(self, message: String) {
        if !self.0.is_null() {
            let error = Box::into_raw(Box::new(ScahError { message }));
            // SAFETY: `new` requires a non-NULL slot to stay writable.
            unsafe { self.0.write(error) };
        }
    }
}

/// Run the body of a fallible entry point, containing panics and reporting
/// any failure through `out_error`.
pub(crate) fn guard(out_error: ErrorSlot, body: impl FnOnce() -> Result<()>) -> ScahStatus {
    let failure = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => return ScahStatus::Ok,
        Ok(Err(failure)) => failure,
        Err(payload) => Failure::new(ScahStatus::InternalPanic, panic_message(payload)),
    };
    out_error.report(failure.message);
    failure.status
}

/// Run an infallible entry point, returning `fallback` if it panics.
pub(crate) fn guard_value<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    match detail {
        Some(detail) => format!("internal panic in scah: {detail}"),
        None => "internal panic in scah".to_owned(),
    }
}

/// Borrow `ptr` as a reference, failing with `NullPointer` when it is NULL.
///
/// # Safety
///
/// A non-NULL `ptr` must point to a live `T` that outlives `'a`.
pub(crate) unsafe fn deref<'a, T>(ptr: *const T, argument: &str) -> Result<&'a T> {
    // SAFETY: guaranteed by the caller.
    unsafe { ptr.as_ref() }.ok_or_else(|| Failure::null(argument))
}

/// Check an out-pointer, failing with `NullPointer` when it is NULL.
pub(crate) fn out<T>(ptr: *mut T, argument: &str) -> Result<*mut T> {
    if ptr.is_null() {
        Err(Failure::null(argument))
    } else {
        Ok(ptr)
    }
}

/// Check a handle out-pointer and set it to NULL before any other work.
///
/// # Safety
///
/// A non-NULL `ptr` must be valid for writing one pointer.
pub(crate) unsafe fn out_handle<T>(ptr: *mut *mut T, argument: &str) -> Result<*mut *mut T> {
    let ptr = out(ptr, argument)?;
    // SAFETY: guaranteed by the caller.
    unsafe { ptr.write(ptr::null_mut()) };
    Ok(ptr)
}

/// Borrow the message of `error`.
///
/// The view stays valid until `scah_error_free(error)`. A NULL `error` yields
/// an empty view.
///
/// # Safety
///
/// `error` must be NULL or a live error returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_error_message(error: *const ScahError) -> ScahStringView {
    guard_value(ScahStringView::EMPTY, || {
        // SAFETY: the caller guarantees `error` is NULL or live.
        match unsafe { error.as_ref() } {
            Some(error) => ScahStringView::new(&error.message),
            None => ScahStringView::EMPTY,
        }
    })
}

/// Free an error. NULL is ignored.
///
/// # Safety
///
/// `error` must be NULL or an error returned by this library that has not
/// been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scah_error_free(error: *mut ScahError) {
    guard_value((), || {
        if !error.is_null() {
            // SAFETY: the caller transfers ownership of a live error.
            drop(unsafe { Box::from_raw(error) });
        }
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Take ownership of an error and return its message.
    pub(crate) fn take_message(error: *mut ScahError) -> String {
        assert!(!error.is_null(), "expected an error");
        // SAFETY: the error is live until freed below, and the view borrows it.
        let message = unsafe { scah_error_message(error).to_str("message") }
            .unwrap()
            .to_owned();
        // SAFETY: the test owns the error and never uses it again.
        unsafe { scah_error_free(error) };
        message
    }

    /// Run `body` through `guard`, returning the status and any message.
    fn run(body: impl FnOnce() -> Result<()>) -> (ScahStatus, Option<String>) {
        let mut error = ptr::dangling_mut::<ScahError>();
        // SAFETY: `error` is a writable local that outlives the slot.
        let status = guard(unsafe { ErrorSlot::new(&mut error) }, body);
        (status, (!error.is_null()).then(|| take_message(error)))
    }

    #[test]
    fn success_clears_out_error() {
        assert_eq!(run(|| Ok(())), (ScahStatus::Ok, None));
    }

    #[test]
    fn failure_reports_status_and_message() {
        let (status, message) = run(|| Err(Failure::null("store")));
        assert_eq!(status, ScahStatus::NullPointer);
        assert_eq!(message.as_deref(), Some("`store` must not be NULL"));

        // SAFETY: a NULL slot is never written.
        let slot = unsafe { ErrorSlot::new(ptr::null_mut()) };
        let status = guard(slot, || Err(Failure::null("store")));
        assert_eq!(status, ScahStatus::NullPointer);
    }

    #[test]
    fn panics_become_internal_panic() {
        let (status, message) = run(|| panic!("boom"));
        assert_eq!(status, ScahStatus::InternalPanic);
        assert_eq!(message.as_deref(), Some("internal panic in scah: boom"));

        let detail = 7;
        let (status, message) = run(|| panic!("boom {detail}"));
        assert_eq!(status, ScahStatus::InternalPanic);
        assert_eq!(message.as_deref(), Some("internal panic in scah: boom 7"));

        let (_, message) = run(|| std::panic::panic_any(detail));
        assert_eq!(message.as_deref(), Some("internal panic in scah"));

        assert_eq!(guard_value(1, || panic!("boom")), 1);
    }

    #[test]
    fn null_error_is_tolerated() {
        // SAFETY: NULL is accepted by both functions.
        unsafe {
            assert_eq!(scah_error_message(ptr::null()).len, 0);
            scah_error_free(ptr::null_mut());
        }
    }
}
