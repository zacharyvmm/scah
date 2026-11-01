pub(crate) fn emit_trace_event(event: &crate::debug::TraceEvent<'_, '_>) {
    tracing::debug!(target: "scah::trace", ?event);
}
