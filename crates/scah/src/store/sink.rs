use std::ops::Range;

use super::{Arena, AttributeId, ElementId, Store, TextStore};
use crate::{Attribute, QuerySection, XHtmlElement};

/// Destination for the matches a parse produces.
///
/// The parser and query engine write results only through this trait, so the
/// same streaming code can fill different result layouts. Every method is a
/// thin, inlined forward for [`Store`], so the ordinary `parse` path
/// monomorphizes to the code it compiled to before the trait existed.
///
/// The attribute and text tapes are shared building blocks: the parser
/// appends to them directly and a sink only records ranges into them.
pub(crate) trait ResultSink<'html, 'query: 'html> {
    /// Record `element` as a match of `selection` nested under `from`
    /// (`ElementId::default()` for a root match) and return its id.
    ///
    /// When `selection.save.attributes` is set, `element.attributes` must be
    /// a subslice of [`ResultSink::attribute_tape`].
    fn push(
        &mut self,
        from: ElementId,
        selection: &QuerySection<'query>,
        element: XHtmlElement<'html>,
    ) -> ElementId;

    /// Attach deferred content to an element once it closes.
    fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    );

    /// Persistent attribute tape the parser appends selected attributes to.
    fn attribute_tape(&mut self) -> &mut Arena<Attribute<'html>, AttributeId>;

    /// Number of attributes currently on the tape.
    fn attribute_count(&self) -> usize;

    /// Shared raw-text and normalized-text tapes.
    fn text_tapes(&self) -> &TextStore;

    /// Mutable access to the shared text tapes for appending.
    fn text_tapes_mut(&mut self) -> &mut TextStore;

    /// Number of matched elements recorded so far.
    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    fn element_count(&self) -> usize;

    /// Number of query-result lists recorded so far.
    #[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
    fn query_node_count(&self) -> usize;

    /// Tag name of a recorded element, for trace events.
    #[cfg(any(debug_assertions, test))]
    fn element_name(&self, element_id: ElementId) -> &'html str;

    /// Record a debug trace event. Sinks may discard events.
    #[cfg(any(debug_assertions, test))]
    fn trace_event(&mut self, event: crate::debug::TraceEvent<'html, 'query>);
}

impl<'html, 'query: 'html> ResultSink<'html, 'query> for Store<'html, 'query> {
    #[inline(always)]
    fn push(
        &mut self,
        from: ElementId,
        selection: &QuerySection<'query>,
        element: XHtmlElement<'html>,
    ) -> ElementId {
        Store::push(self, from, selection, element)
    }

    #[inline(always)]
    fn set_content(
        &mut self,
        element_id: ElementId,
        inner_html: Option<&'html str>,
        raw_text: Option<Range<usize>>,
        text: Option<Range<usize>>,
    ) {
        Store::set_content(self, element_id, inner_html, raw_text, text);
    }

    #[inline(always)]
    fn attribute_tape(&mut self) -> &mut Arena<Attribute<'html>, AttributeId> {
        &mut self.attributes
    }

    #[inline(always)]
    fn attribute_count(&self) -> usize {
        self.attributes.len()
    }

    #[inline(always)]
    fn text_tapes(&self) -> &TextStore {
        &self.text
    }

    #[inline(always)]
    fn text_tapes_mut(&mut self) -> &mut TextStore {
        &mut self.text
    }

    #[inline(always)]
    fn element_count(&self) -> usize {
        self.elements.len()
    }

    #[inline(always)]
    fn query_node_count(&self) -> usize {
        self.queries.len()
    }

    #[cfg(any(debug_assertions, test))]
    #[inline(always)]
    fn element_name(&self, element_id: ElementId) -> &'html str {
        self.elements[element_id].name
    }

    #[cfg(any(debug_assertions, test))]
    #[inline(always)]
    fn trace_event(&mut self, event: crate::debug::TraceEvent<'html, 'query>) {
        Store::trace_event(self, event);
    }
}
