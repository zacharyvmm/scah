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
// feature emit it to `tracing`. Without the `trace` feature (or `cfg(test)`)
// it expands to nothing: neither argument is evaluated, so `$trace` may name
// a field only traced builds have.
#[cfg(any(feature = "trace", test))]
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

#[cfg(not(any(feature = "trace", test)))]
#[macro_export]
macro_rules! scah_trace {
    ($trace:expr, $event:expr) => {{}};
}
