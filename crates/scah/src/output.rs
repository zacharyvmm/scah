//! What a parse writes: the store it fills and, in debug and test builds,
//! the trace of how the parse went.

#[cfg(any(debug_assertions, test))]
use crate::debug::{TraceEvent, TraceStore};
use scah_query_ir::{Program, SectionIndex};
use scah_store::{Content, ElementId, Section, StoreBuilder};
use std::ops::Range;

/// The store being filled, and the trace. The parser owns it and lends it
/// to the matcher and capture, which save rows and content to the store and
/// trace what they do with [`scah_trace!`](crate::scah_trace).
#[derive(Debug)]
pub(crate) struct Output<'html, 'query> {
    pub(crate) store: StoreBuilder<'html, 'query>,
    #[cfg(any(debug_assertions, test))]
    pub(crate) trace: TraceStore<'html, 'query>,
}

impl<'html, 'query: 'html> Output<'html, 'query> {
    /// `store`, set up for the sections of `program`, and a trace with room
    /// for `trace_capacity` events.
    pub(crate) fn new(
        mut store: StoreBuilder<'html, 'query>,
        program: &Program<'query>,
        trace_capacity: usize,
    ) -> Self {
        store.set_sections(&sections(program));
        #[cfg(not(any(debug_assertions, test)))]
        let _ = trace_capacity;
        Self {
            store,
            #[cfg(any(debug_assertions, test))]
            trace: TraceStore::with_capacity(trace_capacity),
        }
    }

    /// Record the content of `element_id`, once its element closed.
    #[inline(always)]
    pub(crate) fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    ) {
        #[cfg(any(debug_assertions, test))]
        let event = TraceEvent::ContentFinalized {
            element_id,
            tag: self.store.name(element_id),
            has_inner_html: inner_html.is_some(),
            has_raw_text: raw_text.is_some(),
            has_text: text.is_some(),
        };
        self.store
            .set_content(element_id, inner_html, raw_text, text);
        crate::scah_trace!(self.trace, event);
    }
}

/// The store's description of `program`'s sections, in program order.
pub(crate) fn sections<'query>(program: &Program<'query>) -> Vec<Section<'query>> {
    (0..program.section_count())
        .map(|index| {
            let section = SectionIndex(index as u32);
            let spec = program.section(section);
            Section {
                selector: spec.source,
                parent: program.section_parent(section).map(|parent| parent.0),
                content: Content {
                    inner_html: spec.save.inner_html,
                    raw_text: spec.save.raw_text,
                    text: spec.save.text,
                },
            }
        })
        .collect()
}
