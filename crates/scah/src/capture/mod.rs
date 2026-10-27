//! Content capture: inner HTML, raw text and normalized text of saved
//! elements.
//!
//! The matcher decides which elements are saved; this module records what
//! they contain. A saved element's content is only known at its close tag, so
//! [`Capture::open`] notes where its content starts and [`Capture::close`]
//! writes the finished ranges to the store.
//!
//! Text between tags is copied to the store's text tapes while at least one
//! saved element wants it: the parser reports each tag's start with
//! [`Capture::text_until`] and the end of each tag with [`Capture::text_from`].
//!
//! Every method takes the parser's `CAPTURE` flag. When it is `false` the parse
//! captures no text, and the text paths compile away.

mod entities;
mod text;

#[cfg(feature = "bench-internals")]
pub use text::TextPathStats;
pub(crate) use text::{
    ParserTextState, PendingSeparator, TextCaptureMode, TextEdgePolicy, TextElementBehavior,
    TextElementFlags,
};

use crate::Reader;
use crate::XHtmlElement;
use crate::engine::DepthSize;
use crate::engine::matcher::SaveHit;
use crate::store::{ElementId, Store, trim_collapsed_range};
use crate::tag::TextTagFlags;
use scah_query_ir::TextRequirements;

/// Captures the content of saved elements while the document is parsed.
#[derive(Debug)]
pub(crate) struct Capture {
    /// Saved elements still open, innermost last.
    pending: Vec<PendingContent>,
    mode: TextCaptureMode,
    pub(crate) text: ParserTextState,
    /// Start of the source text not yet copied to the raw-text tape.
    raw_source_start: Option<usize>,
    /// Open saved elements capturing raw text.
    raw_active: usize,
    /// Open saved elements capturing normalized text.
    text_active: usize,
}

/// Text behavior of an element being opened, decided before it is pushed on
/// the open-element stack.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpenText {
    tag: TextTagFlags,
    /// `None` when the parse does not capture normalized text.
    behavior: Option<TextElementBehavior>,
    edge: TextEdgePolicy,
}

impl OpenText {
    /// Flags the open-element stack keeps for [`Capture::close`].
    #[inline]
    pub(crate) fn stack_flags(&self) -> TextElementFlags {
        self.behavior
            .map(|behavior| behavior.stack_flags(self.tag))
            .unwrap_or_else(TextElementFlags::empty)
    }
}

impl Capture {
    pub(crate) fn new(requirements: TextRequirements) -> Self {
        let text = ParserTextState::new(requirements);
        Self {
            pending: Vec::new(),
            mode: text.mode,
            text,
            raw_source_start: None,
            raw_active: 0,
            text_active: 0,
        }
    }

    /// Whether the parse captures raw or normalized text.
    #[inline(always)]
    pub(crate) fn captures_any(&self) -> bool {
        self.mode.captures_any()
    }

    #[inline(always)]
    fn captures_text<const CAPTURE: bool>(&self) -> bool {
        CAPTURE && self.mode.captures_text()
    }

    #[inline(always)]
    fn text_active<const CAPTURE: bool>(&self) -> bool {
        CAPTURE && (self.raw_active > 0 || self.text_active > 0)
    }

    /// Whether no saved element is waiting for its close tag.
    #[inline(always)]
    pub(crate) fn idle(&self) -> bool {
        self.pending.is_empty()
    }

    /// The text classification of a tag, when normalized text needs it.
    #[inline(always)]
    pub(crate) fn text_tag<const CAPTURE: bool>(&self, tag: TextTagFlags) -> TextTagFlags {
        if self.captures_text::<CAPTURE>() {
            tag
        } else {
            TextTagFlags::default()
        }
    }

    /// Whether open tags must be checked for the `hidden` attribute.
    #[inline(always)]
    pub(crate) fn needs_hidden<const CAPTURE: bool>(&self) -> bool {
        self.captures_text::<CAPTURE>()
    }

    /// Copy the source text from the last tag up to `end` to the tapes of
    /// the active captures.
    #[inline(always)]
    pub(crate) fn text_until<const CAPTURE: bool>(
        &mut self,
        reader: &Reader<'_>,
        end: usize,
        depth: DepthSize,
        store: &mut Store<'_, '_>,
    ) {
        if self.text_active::<CAPTURE>() {
            self.flush_source_text(reader, end, depth, store);
        }
    }

    /// A tag that is not an element (a comment or declaration) ends a
    /// preformatted element's leading newline.
    #[inline(always)]
    pub(crate) fn cancel_initial_newline<const CAPTURE: bool>(&mut self) {
        if self.captures_text::<CAPTURE>() {
            self.text.cancel_initial_newline();
        }
    }

    /// Source text from `position` on belongs to the active captures.
    #[inline(always)]
    pub(crate) fn text_from<const CAPTURE: bool>(&mut self, position: usize) {
        if CAPTURE && self.captures_any() {
            if self.raw_active > 0 {
                self.raw_source_start = Some(position);
            }
            if self.text_active > 0 {
                self.text.mark_source_start(position);
            }
        }
    }

    fn flush_source_text(
        &mut self,
        reader: &Reader<'_>,
        end: usize,
        depth: DepthSize,
        store: &mut Store<'_, '_>,
    ) {
        let raw_start = self.raw_source_start.take();
        let text_start = self.text.source_start.take();
        if raw_start.is_none() && text_start.is_none() {
            return;
        }
        #[cfg(feature = "bench-internals")]
        {
            self.text.path_stats.flush_calls += 1;
        }
        if let Some(start) = raw_start.filter(|start| *start < end) {
            store.text.raw_text.push_str(reader.slice(start..end));
        }
        if let Some(start) = text_start.filter(|start| *start < end) {
            self.text.write_normalized_fragment(
                &mut store.text.text,
                reader.slice(start..end),
                depth,
            );
        }
    }

    /// Decide the text behavior of `element` before it is pushed.
    #[inline(always)]
    pub(crate) fn open_text<const CAPTURE: bool>(
        &mut self,
        element: &XHtmlElement<'_>,
        tag: TextTagFlags,
    ) -> OpenText {
        if !self.captures_text::<CAPTURE>() {
            return OpenText {
                tag,
                behavior: None,
                edge: TextEdgePolicy::TrimCollapsedSeparators,
            };
        }
        let hidden = element_has_hidden(element, &mut self.text);
        let behavior = text_behavior_for(tag, hidden, &mut self.text);
        let edge = self.text.edge_policy_for_child(behavior, tag.is_cell());
        self.text.cancel_initial_newline();
        OpenText {
            tag,
            behavior: Some(behavior),
            edge,
        }
    }

    /// Start capturing the content of the elements the matcher just saved.
    ///
    /// `inner_html_start` is the end of the open tag and `depth` the
    /// element's depth. A void element's content is empty, so it is written
    /// at once.
    #[inline(always)]
    pub(crate) fn open<'html, 'query: 'html, const CAPTURE: bool>(
        &mut self,
        hits: &[SaveHit],
        open: &OpenText,
        void: bool,
        inner_html_start: usize,
        depth: DepthSize,
        store: &mut Store<'html, 'query>,
    ) {
        let text_was_active = CAPTURE && self.text_active > 0;
        let (new_raw, new_text) = if CAPTURE && !void {
            hits.iter().fold((0, 0), |(raw, text), hit| {
                (
                    raw + usize::from(hit.save_raw_text),
                    text + usize::from(hit.save_text),
                )
            })
        } else {
            (0, 0)
        };
        if let Some(behavior) = open.behavior
            && (text_was_active || new_text > 0)
        {
            if !text_was_active {
                self.text.discard_pending();
            }
            self.text
                .before_open_element(&mut store.text.text, behavior, void);
            self.text.before_text_range_start(
                &mut store.text.text,
                behavior,
                open.edge,
                open.tag.is_cell(),
            );
        }
        let (raw_start, text_start) = if CAPTURE {
            (store.text.raw_text.len(), store.text.text.len())
        } else {
            (0, 0)
        };

        if void {
            for hit in hits.iter().filter(|hit| hit.needs_close_finalization()) {
                store.set_content(
                    hit.element_id,
                    None,
                    hit.save_raw_text.then_some(raw_start..raw_start),
                    hit.save_text.then_some(text_start..text_start),
                );
            }
            if let Some(behavior) = open.behavior
                && text_was_active
                && !behavior.suppressed
                && open.tag.is_break()
            {
                self.text.queue_separator(PendingSeparator::LineBreak);
            }
            return;
        }

        for hit in hits.iter().filter(|hit| hit.needs_close_finalization()) {
            self.pending.push(PendingContent::new(
                hit.element_id,
                depth,
                hit.save_inner_html.then_some(inner_html_start),
                hit.save_raw_text.then_some(raw_start),
                hit.save_text.then_some(text_start),
                open.edge,
            ));
        }
        if CAPTURE {
            self.raw_active += new_raw;
            self.text_active += new_text;
        }
        if let Some(behavior) = open.behavior {
            self.text.after_open_element(behavior, depth);
        }
    }

    /// Finish the content of the saved elements closing at `depth`.
    ///
    /// `inner_html_end` is the start of the close tag, and `text_flags` the
    /// flags [`OpenText::stack_flags`] gave the element.
    #[inline(always)]
    pub(crate) fn close<'html, 'query: 'html, const CAPTURE: bool>(
        &mut self,
        text_flags: TextElementFlags,
        depth: DepthSize,
        inner_html_end: usize,
        reader: &Reader<'html>,
        store: &mut Store<'html, 'query>,
    ) {
        debug_assert!(
            self.pending
                .last()
                .is_none_or(|content| content.depth <= depth),
            "pending content outlived its element"
        );
        // Most elements saved nothing: one comparison rules that out.
        if self
            .pending
            .last()
            .is_some_and(|content| content.depth == depth)
        {
            self.finish_pending::<CAPTURE>(depth, inner_html_end, reader, store);
        }
        if self.captures_text::<CAPTURE>() {
            self.text.after_close_element(text_flags, depth);
            if self.text_active == 0 {
                self.text.discard_pending();
            } else if !text_flags.contains(TextElementFlags::SUPPRESSED) {
                if text_flags.contains(TextElementFlags::CELL) {
                    self.text.queue_cell_boundary();
                } else if let Some(separator) = text_flags.post_text_separator() {
                    self.text.queue_separator(separator);
                }
            }
        }
    }

    /// Finish the pending records of the element closing at `depth`, which
    /// are the innermost ones, in the order they were saved.
    #[inline]
    fn finish_pending<'html, 'query: 'html, const CAPTURE: bool>(
        &mut self,
        depth: DepthSize,
        inner_html_end: usize,
        reader: &Reader<'html>,
        store: &mut Store<'html, 'query>,
    ) {
        let start = self
            .pending
            .iter()
            .rposition(|content| content.depth != depth)
            .map_or(0, |index| index + 1);
        let (mut raw, mut text) = (0, 0);
        for content in &self.pending[start..] {
            let (raw_text, normalized) =
                finish_content::<CAPTURE>(content, inner_html_end, reader, store);
            raw += usize::from(raw_text);
            text += usize::from(normalized);
        }
        self.pending.truncate(start);
        debug_assert!(raw <= self.raw_active && text <= self.text_active);
        self.raw_active -= raw;
        self.text_active -= text;
    }

    #[cfg(test)]
    pub(crate) fn active_counts(&self) -> (usize, usize) {
        (self.raw_active, self.text_active)
    }
}

/// Write the finished ranges of `content` to the store. Returns whether it
/// captured raw text and normalized text.
#[inline(always)]
fn finish_content<'html, 'query: 'html, const CAPTURE: bool>(
    content: &PendingContent,
    inner_html_end: usize,
    reader: &Reader<'html>,
    store: &mut Store<'html, 'query>,
) -> (bool, bool) {
    let inner_html = content
        .inner_html_start()
        .map(|start| reader.slice(start..inner_html_end));
    if !CAPTURE {
        store.set_content(content.element_id, inner_html, None, None);
        return (false, false);
    }
    let raw_text = content
        .raw_text_start()
        .map(|start| start..store.text.raw_text.len());
    let text = content.text_start().map(|start| {
        let range = start..store.text.text.len();
        match content.text_edge_policy() {
            TextEdgePolicy::TrimCollapsedSeparators => {
                trim_collapsed_range(&store.text.text, range)
            }
            TextEdgePolicy::Preserve => range,
        }
    });
    let captured = (raw_text.is_some(), text.is_some());
    store.set_content(content.element_id, inner_html, raw_text, text);
    captured
}

#[inline]
fn element_has_hidden(element: &XHtmlElement<'_>, text: &mut ParserTextState) -> bool {
    #[cfg(feature = "bench-internals")]
    {
        text.path_stats.hidden_attribute_scans += 1;
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = text;
    element
        .attributes
        .iter()
        .any(|attr| attr.key.eq_ignore_ascii_case("hidden"))
}

#[inline]
fn text_behavior_for(
    tag: TextTagFlags,
    has_hidden: bool,
    text: &mut ParserTextState,
) -> TextElementBehavior {
    #[cfg(feature = "bench-internals")]
    {
        text.path_stats.normalized_behavior_computations += 1;
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = text;
    let suppressed = tag.is_suppressed() || has_hidden;
    let preformatted = tag.is_preformatted();
    let opening_separator = if suppressed {
        PendingSeparator::None
    } else if tag.is_block() || tag.is_row() {
        PendingSeparator::LineBreak
    } else {
        PendingSeparator::None
    };
    TextElementBehavior {
        suppressed,
        preformatted,
        opening_separator,
    }
}

/// Sentinel for an inactive deferred content-start offset.
///
/// A start of `usize::MAX` cannot be a valid tape or source index: Rust
/// allocations cannot have length `usize::MAX`, and reader/tape lengths must
/// remain representable and indexable. Any input near this limit would fail
/// allocation long before a range could be created.
const NO_START: usize = usize::MAX;

/// Where the content of an open saved element starts.
///
/// Optional start offsets are packed as plain `usize` values with [`NO_START`]
/// for `None`, so inactive modes (e.g. inner-HTML-only) do not pay for three
/// `Option<usize>` niches (typically 16 bytes each on 64-bit).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingContent {
    pub element_id: ElementId,
    /// Depth of the element; its close tag finishes the content.
    depth: DepthSize,
    inner_html_start: usize,
    raw_text_start: usize,
    text_start: usize,
    text_edge_policy: TextEdgePolicy,
}

impl PendingContent {
    #[inline]
    pub(crate) fn new(
        element_id: ElementId,
        depth: DepthSize,
        inner_html_start: Option<usize>,
        raw_text_start: Option<usize>,
        text_start: Option<usize>,
        text_edge_policy: TextEdgePolicy,
    ) -> Self {
        Self {
            element_id,
            depth,
            inner_html_start: inner_html_start.unwrap_or(NO_START),
            raw_text_start: raw_text_start.unwrap_or(NO_START),
            text_start: text_start.unwrap_or(NO_START),
            text_edge_policy,
        }
    }

    #[inline]
    pub(crate) fn inner_html_start(&self) -> Option<usize> {
        (self.inner_html_start != NO_START).then_some(self.inner_html_start)
    }

    #[inline]
    pub(crate) fn raw_text_start(&self) -> Option<usize> {
        (self.raw_text_start != NO_START).then_some(self.raw_text_start)
    }

    #[inline]
    pub(crate) fn text_start(&self) -> Option<usize> {
        (self.text_start != NO_START).then_some(self.text_start)
    }

    #[inline]
    pub(crate) fn text_edge_policy(&self) -> TextEdgePolicy {
        self.text_edge_policy
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingContent, TextEdgePolicy};
    use crate::store::ElementId;

    #[test]
    fn pending_content_none_starts_round_trip() {
        let content = PendingContent::new(
            ElementId::from(0usize),
            1,
            None,
            None,
            None,
            TextEdgePolicy::TrimCollapsedSeparators,
        );

        assert_eq!(content.inner_html_start(), None);
        assert_eq!(content.raw_text_start(), None);
        assert_eq!(content.text_start(), None);
        assert_eq!(
            content.text_edge_policy(),
            TextEdgePolicy::TrimCollapsedSeparators
        );
    }

    #[test]
    fn pending_content_present_starts_round_trip() {
        let content = PendingContent::new(
            ElementId::from(1usize),
            1,
            Some(10),
            Some(20),
            Some(30),
            TextEdgePolicy::Preserve,
        );

        assert_eq!(content.inner_html_start(), Some(10));
        assert_eq!(content.raw_text_start(), Some(20));
        assert_eq!(content.text_start(), Some(30));
        assert_eq!(content.text_edge_policy(), TextEdgePolicy::Preserve);
    }

    #[test]
    fn pending_content_mixed_optional_starts_round_trip() {
        let cases = [
            (Some(1), None, None),
            (None, Some(2), None),
            (None, None, Some(3)),
            (Some(4), Some(5), None),
            (Some(6), None, Some(7)),
            (None, Some(8), Some(9)),
            (Some(10), Some(11), Some(12)),
            (None, None, None),
        ];
        for (inner, raw, text) in cases {
            let content = PendingContent::new(
                ElementId::from(0usize),
                1,
                inner,
                raw,
                text,
                TextEdgePolicy::Preserve,
            );
            assert_eq!(content.inner_html_start(), inner);
            assert_eq!(content.raw_text_start(), raw);
            assert_eq!(content.text_start(), text);
        }
    }

    #[test]
    fn pending_content_layout_is_compact_on_64bit() {
        use std::mem::size_of;
        // On 64-bit, sentinel-packed starts keep the deferred record at about
        // 40 bytes (at most 48 with alignment padding).
        if size_of::<usize>() == 8 {
            let size = size_of::<PendingContent>();
            assert!(
                size <= 48,
                "PendingContent grew unexpectedly: {size} bytes (expected <= 48)"
            );
        }
    }
}
