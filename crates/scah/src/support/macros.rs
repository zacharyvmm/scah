#[macro_export]
macro_rules! mut_prt_unchecked {
    ($e:expr) => {{
        #[inline(always)]
        fn cast<T>(r: &T) -> *mut T {
            r as *const T as *mut T
        }
        cast($e)
    }};
}

// Record a `TraceEvent` in `$trace`, a `TraceStore`, and with the `otel`
// feature emit it to `tracing`. Release builds expand to nothing: neither
// argument is evaluated, so `$trace` may name a field only debug builds have.
#[cfg(any(debug_assertions, test))]
#[macro_export]
macro_rules! scah_trace {
    ($trace:expr, $event:expr) => {{
        let event = $event;
        #[cfg(feature = "otel")]
        {
            $crate::otel::emit_trace_event(&event);
        }
        $trace.push(event);
    }};
}

#[cfg(not(any(debug_assertions, test)))]
#[macro_export]
macro_rules! scah_trace {
    ($trace:expr, $event:expr) => {{}};
}
