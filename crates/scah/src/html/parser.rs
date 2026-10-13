use super::element::builder::XHtmlTag;
use super::indexer::{AutoTagIndexer, IndexingMode, TagEvent, TagIndexer, TagKind};
use super::open_elements::{OpenElement, OpenElementStack, SavedElement};
use super::tag::{ClassifiedTag, TextTagFlags};
use super::text_edge::TextEdgePolicy;
use super::text_state::{
    ParserTextState, PendingSeparator, TextCaptureMode, TextElementBehavior, TextElementFlags,
};
use crate::Attribute;
use crate::ParseError;
use crate::Reader;
use crate::StructuralMatchContext;
use crate::XHtmlElement;
use crate::debug::ImpliedCloseReason;
#[cfg(any(debug_assertions, test))]
use crate::debug::TraceEvent;
use crate::engine::attribute_interest::AttributeInterest;
use crate::engine::matcher::{AnyMatcher, SaveHit};
use crate::engine::{DepthSize, MAX_ELEMENT_DEPTH};
use crate::store::{Store, trim_collapsed_range};
use crate::{LocalSelectorList, Program, QuerySpec};
use scah_query_ir::AttributeMask;
use scah_query_ir::TagId;
use smallvec::SmallVec;
use std::borrow::Cow;

/// Where the parser is in the document.
#[derive(Debug, Default)]
struct DocumentPosition {
    /// End of the current open tag (inner HTML start) on open events, start
    /// of the current close tag (inner HTML end) on close events.
    reader_position: usize,
    element_depth: DepthSize,
}

#[derive(Default)]
struct ParserTempState<'html, 'query> {
    closing_elements: Vec<OpenElement<'html>>,
    implied_closes: Vec<OpenElement<'html>>,
    saved_elements: Vec<SavedElement>,
    /// Attributes of the open tag being parsed, cleared for each tag.
    attributes: Vec<Attribute<'html>>,
    save_hits: Vec<SaveHit>,
    attribute_interest: AttributeInterest<'query>,
    structural: Option<Box<StructuralParserState<'html, 'query>>>,
}

type TypeCounts<'html> = SmallVec<[(&'html str, u32); 4]>;

struct StructuralParserState<'html, 'query> {
    child_counts: Option<Vec<u32>>,
    type_counts: Option<Vec<TypeCounts<'html>>>,
    tracked_type_names: Option<Vec<&'query str>>,
    filters: Vec<(&'query LocalSelectorList<'query>, Vec<u32>)>,
    attribute_interest: AttributeMask,
    root_seen: bool,
}

impl<'html, 'query> StructuralParserState<'html, 'query> {
    fn new(program: &Program<'query>) -> Self {
        let features = program.features();
        let filters: Vec<_> = program
            .structural_filters()
            .into_iter()
            // Filter lists are evaluated without a structural context. The
            // selector parser rejects structural pseudo-classes inside `of S`,
            // but hand-built predicates can still contain them. Leaving such a
            // filter uncounted makes its ordinal fail closed instead of
            // counting siblings that do not match.
            .filter(|filter| {
                !filter
                    .as_slice()
                    .iter()
                    .any(|predicate| predicate.requires_structural())
            })
            .collect();
        Self {
            // Keep a virtual document parent so fragment roots participate in
            // ordinal selectors consistently.
            child_counts: features.needs_child_ordinals.then(|| vec![0]),
            type_counts: features.needs_type_ordinals.then(|| vec![SmallVec::new()]),
            tracked_type_names: features
                .needs_type_ordinals
                .then(|| program.type_ordinal_names())
                .flatten(),
            filters: filters
                .into_iter()
                .map(|filter| (filter, vec![0]))
                .collect(),
            attribute_interest: program.filter_interest(),
            root_seen: false,
        }
    }

    fn open(
        &mut self,
        name: &'html str,
        persistent: bool,
        element: &XHtmlElement<'html>,
    ) -> StructuralMatchContext<'query> {
        let is_document_root = !self.root_seen;
        self.root_seen = true;
        let child_index = self.child_counts.as_mut().map_or(0, |counts| {
            if let Some(count) = counts.last_mut() {
                *count = count.saturating_add(1);
                *count
            } else {
                1
            }
        });
        let tracks_type = self.tracked_type_names.as_ref().is_none_or(|names| {
            names
                .iter()
                .any(|tracked| tracked.eq_ignore_ascii_case(name))
        });
        let type_index = self.type_counts.as_mut().map_or(0, |levels| {
            if !tracks_type {
                return 0;
            }
            if let Some(counts) = levels.last_mut() {
                if let Some((_, count)) = counts
                    .iter_mut()
                    .find(|(child_name, _)| child_name.eq_ignore_ascii_case(name))
                {
                    *count = count.saturating_add(1);
                    *count
                } else {
                    counts.push((name, 1));
                    1
                }
            } else {
                1
            }
        });
        let mut filtered_child_indices = SmallVec::new();
        for (filter, counts) in &mut self.filters {
            if filter
                .as_slice()
                .iter()
                .any(|predicate| predicate.matches_local_element_unchecked(element))
            {
                let parent_count = counts.last_mut().expect("filter parent count");
                *parent_count = parent_count.saturating_add(1);
                filtered_child_indices.push((*filter, *parent_count));
            }
        }
        if persistent {
            if let Some(counts) = &mut self.child_counts {
                counts.push(0);
            }
            if let Some(counts) = &mut self.type_counts {
                counts.push(SmallVec::new());
            }
            for (_, counts) in &mut self.filters {
                counts.push(0);
            }
        }
        StructuralMatchContext {
            child_index,
            type_index,
            filtered_child_indices,
            is_document_root,
            is_scope_root: is_document_root,
        }
    }

    fn close(&mut self) {
        if let Some(counts) = &mut self.child_counts {
            counts.pop();
        }
        if let Some(counts) = &mut self.type_counts {
            counts.pop();
        }
        for (_, counts) in &mut self.filters {
            counts.pop();
        }
    }
}

pub struct XHtmlParser<'html, 'query> {
    position: DocumentPosition,
    matcher: AnyMatcher<'query>,
    store: Store<'html, 'query>,
    element: crate::XHtmlElement<'html>,
    open_elements: OpenElementStack<'html>,
    temp_state: ParserTempState<'html, 'query>,
    /// Hot-path capture mode (mirrors `text_state.mode` for cheaper checks).
    capture_mode: TextCaptureMode,
    text_state: ParserTextState,
    raw_source_start: Option<usize>,
    raw_active_count: usize,
    text_active_count: usize,
    /// Every root query is `First`, so parsing may stop early.
    can_finish: bool,
    raw_text_close: Option<&'static str>,
    eof_drained: bool,
    /// The first `next` or `run` bound the store to its reader's source.
    source_bound: bool,
    parse_error: Option<ParseError>,
    indexer: AutoTagIndexer,
    #[cfg(test)]
    attribute_parse_count: usize,
    #[cfg(test)]
    selected_attribute_count: usize,
}

#[inline]
fn element_has_hidden(element: &XHtmlElement<'_>, text_state: &mut ParserTextState) -> bool {
    #[cfg(feature = "bench-internals")]
    {
        text_state.path_stats.hidden_attribute_scans += 1;
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = text_state;
    element
        .attributes
        .iter()
        .any(|attr| attr.key.eq_ignore_ascii_case("hidden"))
}

#[inline]
fn text_behavior_for(
    tag: TextTagFlags,
    has_hidden: bool,
    text_state: &mut ParserTextState,
) -> TextElementBehavior {
    #[cfg(feature = "bench-internals")]
    {
        text_state.path_stats.normalized_behavior_computations += 1;
    }
    #[cfg(not(feature = "bench-internals"))]
    let _ = text_state;
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

impl<'html, 'query: 'html> XHtmlParser<'html, 'query> {
    pub fn new<Q: QuerySpec<'query>>(queries: &'query [Q]) -> Self {
        Self::from_program(Cow::Owned(Program::compile(queries)), None, None)
    }

    /// Like [`XHtmlParser::new`], but reserves result storage for an input of
    /// `capacity` bytes.
    pub fn with_capacity<Q: QuerySpec<'query>>(queries: &'query [Q], capacity: usize) -> Self {
        Self::from_program(Cow::Owned(Program::compile(queries)), Some(capacity), None)
    }

    pub(crate) fn from_program(
        program: Cow<'query, Program<'query>>,
        capacity: Option<usize>,
        indexing_mode: Option<IndexingMode>,
    ) -> Self {
        let features = *program.features();
        let requirements = features.text;
        let text_state = ParserTextState::new(requirements);
        let parse_attributes = features.parses_attributes || requirements.text;
        let indexing_mode = indexing_mode.unwrap_or(if features.all_roots_first {
            IndexingMode::Rolling
        } else {
            IndexingMode::FullDocument
        });
        let store = capacity.map_or_else(Store::default, |capacity| {
            Store::with_capacity_requirements(
                capacity,
                crate::CapacityOptions {
                    reserve_raw_text: requirements.raw_text,
                    reserve_text: requirements.text,
                    ..crate::CapacityOptions::default()
                },
                features.stores_attributes,
                false,
            )
        });
        let structural = features
            .has_structural
            .then(|| Box::new(StructuralParserState::new(&program)));
        let attribute_interest = AttributeInterest::new(program.attribute_names());
        let mut store = store;
        store.set_sections(&program);

        Self {
            position: DocumentPosition::default(),
            matcher: AnyMatcher::new(program),
            element: XHtmlElement::default(),
            open_elements: OpenElementStack::default(),
            temp_state: ParserTempState {
                attribute_interest,
                structural,
                ..ParserTempState::default()
            },
            capture_mode: text_state.mode,
            text_state,
            raw_source_start: None,
            raw_active_count: 0,
            text_active_count: 0,
            can_finish: features.all_roots_first,
            raw_text_close: None,
            eof_drained: false,
            source_bound: false,
            parse_error: None,
            indexer: AutoTagIndexer::new(indexing_mode, parse_attributes),
            #[cfg(test)]
            attribute_parse_count: 0,
            #[cfg(test)]
            selected_attribute_count: 0,
            store,
        }
    }

    fn flush_source_text(&mut self, reader: &Reader<'html>, end: usize) {
        let raw_start = self.raw_source_start.take();
        let text_start = self.text_state.source_start.take();
        if raw_start.is_none() && text_start.is_none() {
            return;
        }
        #[cfg(feature = "bench-internals")]
        {
            self.text_state.path_stats.flush_calls += 1;
        }
        if let Some(start) = raw_start.filter(|start| *start < end) {
            self.store.text.raw_text.push_str(reader.slice(start..end));
        }
        if let Some(start) = text_start.filter(|start| *start < end) {
            let depth = self.position.element_depth;
            self.text_state.write_normalized_fragment(
                &mut self.store.text.text,
                reader.slice(start..end),
                depth,
            );
        }
    }

    #[inline]
    fn mark_active_source_start(&mut self, position: usize) {
        if self.raw_active_count > 0 {
            self.raw_source_start = Some(position);
        }
        if self.text_active_count > 0 {
            self.text_state.mark_source_start(position);
        }
    }

    /// Process the next tag. Returns `false` once parsing is complete.
    ///
    /// # Panics
    ///
    /// If `reader` reads a different source than the first call did (the
    /// results borrow from one document), the source is not UTF-8, or it is
    /// 4 GiB or longer.
    pub fn next(&mut self, reader: &mut Reader<'html>) -> bool {
        if self.parse_error.is_some() {
            return false;
        }
        self.bind_source(reader);
        self.indexer.prepare(reader.source());
        if self.capture_mode.captures_any() {
            self.next_mode::<true>(reader)
        } else {
            self.next_mode::<false>(reader)
        }
    }

    pub(crate) fn run(&mut self, reader: &mut Reader<'html>) {
        if self.parse_error.is_some() {
            return;
        }
        self.bind_source(reader);
        // A full run keeps one Reader source, so the index policy only needs
        // preparation once. `next` prepares per call because its caller owns
        // the Reader and may step a different source between calls.
        self.indexer.prepare(reader.source());
        if self.capture_mode.captures_any() {
            while self.next_mode::<true>(reader) {}
        } else {
            self.run_without_text_capture(reader);
        }
    }

    pub(crate) fn run_without_text_capture(&mut self, reader: &mut Reader<'html>) {
        debug_assert!(!self.capture_mode.captures_any());
        if self.parse_error.is_some() {
            return;
        }
        self.bind_source(reader);
        self.indexer.prepare(reader.source());
        while self.next_mode::<false>(reader) {}
    }

    /// Point the store at the document being parsed, which saved strings
    /// borrow from. Later calls must read the same document: the matcher and
    /// open elements carry its state, whether or not any row was saved yet.
    #[inline]
    fn bind_source(&mut self, reader: &Reader<'html>) {
        let source = reader.source();
        if self.source_bound {
            assert!(self.store.bound_to(source), "a parser reads one document");
        } else {
            let html = reader.source_str().expect("a parser reads UTF-8 HTML");
            self.store.set_html(html);
            self.source_bound = true;
        }
    }

    /// Whether no later tag can change the results: every query is done and
    /// no saved element is waiting for its close tag.
    #[inline]
    fn finished(&self) -> bool {
        self.can_finish && self.temp_state.saved_elements.is_empty() && self.matcher.finished()
    }

    #[inline(always)]
    fn next_mode<const CAPTURE: bool>(&mut self, reader: &mut Reader<'html>) -> bool {
        if let Some(close_tag) = self.raw_text_close {
            return self.next_after_raw_text::<CAPTURE>(reader, close_tag);
        }

        let source = reader.source();
        let mut finished = false;
        let mut open_tag_flags = None;
        let mut open_tag_id = TagId::UNKNOWN;
        let mut open_text_tag_flags = TextTagFlags::default();
        let tag = loop {
            let Some(span) = self.indexer.next(source, reader.get_position()) else {
                reader.advance_to(source.len());
                self.drain_open_elements::<CAPTURE>(reader);
                return false;
            };

            match span {
                TagEvent::Complete(span) if span.kind == TagKind::Ignored => {
                    self.position.reader_position = span.start;
                    if CAPTURE && (self.raw_active_count > 0 || self.text_active_count > 0) {
                        self.flush_source_text(reader, self.position.reader_position);
                    }
                    if CAPTURE && self.capture_mode.captures_text() {
                        self.text_state.cancel_initial_newline();
                    }
                    reader.advance_to(span.end);
                    if CAPTURE && self.capture_mode.captures_any() {
                        self.mark_active_source_start(reader.get_position());
                    }
                }
                TagEvent::Open(open) => {
                    self.position.reader_position = open.start;
                    let name = open.name(source);
                    self.element.set_name(name);

                    let tag_id = TagId::of(name);
                    let classified = ClassifiedTag::of(tag_id);
                    let tag_flags = classified.parser;
                    let text_tag_flags = if CAPTURE && self.capture_mode.captures_text() {
                        classified.text
                    } else {
                        TextTagFlags::default()
                    };
                    open_tag_id = tag_id;
                    if CAPTURE && (self.raw_active_count > 0 || self.text_active_count > 0) {
                        self.flush_source_text(reader, open.start);
                    }
                    if tag_flags.can_trigger_implied_close() {
                        self.open_elements
                            .prepare_for_open_into(tag_flags, &mut self.temp_state.implied_closes);
                        if !self.temp_state.implied_closes.is_empty() {
                            finished = self.drain_implied_closes::<CAPTURE>(
                                reader,
                                Some(ImpliedCloseReason::OpenTagRule),
                                None,
                            ) || finished;
                        }
                    }

                    let interest = &mut self.temp_state.attribute_interest;
                    interest.clear();
                    if self.matcher.prepare(tag_id, name) {
                        interest.add(self.matcher.attribute_mask());
                    }
                    if let Some(structural) = self.temp_state.structural.as_ref() {
                        interest.add(structural.attribute_interest);
                    }
                    if CAPTURE && self.capture_mode.captures_text() {
                        interest.require_hidden();
                    }
                    // A tag ending immediately after its name cannot carry
                    // selector attributes, saved attributes, or hidden-text
                    // suppression, so it needs neither tokenizing nor a search
                    // for its end.
                    let end = if source.get(open.attributes_start) == Some(&b'>') {
                        open.attributes_start + 1
                    } else if !interest.is_empty() {
                        #[cfg(test)]
                        {
                            self.attribute_parse_count += 1;
                        }
                        let mut attributes = Reader::from_bytes(&source[open.attributes_start..]);
                        // The store copies the attributes of rows that save
                        // them, so every tag parses into the same scratch list.
                        self.temp_state.attributes.clear();
                        self.element.parse_attributes(
                            &mut attributes,
                            &mut self.temp_state.attributes,
                            &self.temp_state.attribute_interest,
                        );
                        #[cfg(test)]
                        {
                            self.selected_attribute_count += self.element.attributes.len();
                        }
                        open.attributes_start + attributes.get_position()
                    } else {
                        self.indexer.finish_open(source, &open)
                    };
                    reader.advance_to(end);
                    open_tag_flags = Some(tag_flags);
                    open_text_tag_flags = text_tag_flags;
                    break XHtmlTag::Open;
                }
                TagEvent::Complete(span) => {
                    debug_assert_eq!(span.kind, TagKind::Close);
                    self.position.reader_position = span.start;
                    let name = span.name(source);
                    reader.advance_to(span.end);
                    break XHtmlTag::Close(name);
                }
            }
        };
        let tag_start_position = self.position.reader_position;

        match tag {
            XHtmlTag::Open => {
                let tag = open_tag_flags.expect("opening tags are classified before matching");

                if let Some(close_tag) = tag.raw_text_close_tag() {
                    self.raw_text_close = Some(close_tag);
                }

                self.position.reader_position = reader.get_position();

                let is_self_closing = tag.is_void();

                let (text_behavior, text_edge_policy) = if CAPTURE
                    && self.capture_mode.captures_text()
                {
                    let has_hidden = element_has_hidden(&self.element, &mut self.text_state);
                    let behavior =
                        text_behavior_for(open_text_tag_flags, has_hidden, &mut self.text_state);
                    let edge = self
                        .text_state
                        .edge_policy_for_child(behavior, open_text_tag_flags.is_cell());
                    self.text_state.cancel_initial_newline();
                    (Some(behavior), edge)
                } else {
                    (None, TextEdgePolicy::TrimCollapsedSeparators)
                };
                let text_flags = text_behavior
                    .map(|behavior| behavior.stack_flags(open_text_tag_flags))
                    .unwrap_or_else(TextElementFlags::empty);
                if is_self_closing {
                    let depth = self.open_elements.depth().saturating_add(1);
                    if depth > MAX_ELEMENT_DEPTH {
                        self.record_parse_error(ParseError::MaximumDepthExceeded);
                        return false;
                    }
                    self.position.element_depth = depth;
                } else if let Err(err) = self.open_elements.push_classified(
                    self.element.name,
                    open_tag_id,
                    tag,
                    self.temp_state.saved_elements.len(),
                    text_flags,
                ) {
                    self.record_parse_error(err);
                    return false;
                } else {
                    self.position.element_depth = self.open_elements.depth();
                }
                let structural =
                    self.temp_state.structural.as_deref_mut().map(|state| {
                        state.open(self.element.name, !is_self_closing, &self.element)
                    });

                crate::scah_trace!(
                    self.store,
                    TraceEvent::OpenTag {
                        tag: self.element.name,
                        depth: self.position.element_depth,
                        reader_position: self.position.reader_position,
                        self_closing: is_self_closing,
                    }
                );

                self.matcher.open(
                    &self.element,
                    structural.as_ref(),
                    &mut self.store,
                    &mut self.temp_state.save_hits,
                );
                let text_was_active = CAPTURE && self.text_active_count > 0;
                let (new_raw_count, new_text_count) = if CAPTURE && !is_self_closing {
                    self.temp_state
                        .save_hits
                        .iter()
                        .fold((0, 0), |(raw, text), hit| {
                            (
                                raw + usize::from(hit.save_raw_text),
                                text + usize::from(hit.save_text),
                            )
                        })
                } else {
                    (0, 0)
                };
                if let Some(behavior) = text_behavior
                    && (text_was_active || new_text_count > 0)
                {
                    if !text_was_active {
                        self.text_state.discard_pending();
                    }
                    self.text_state.before_open_element(
                        &mut self.store.text.text,
                        behavior,
                        is_self_closing,
                    );
                    self.text_state.before_text_range_start(
                        &mut self.store.text.text,
                        behavior,
                        text_edge_policy,
                        open_text_tag_flags.is_cell(),
                    );
                }
                let (raw_start, text_start) = if CAPTURE {
                    (self.store.text.raw_text.len(), self.store.text.text.len())
                } else {
                    (0, 0)
                };
                if is_self_closing {
                    for hit in &self.temp_state.save_hits {
                        if hit.needs_close_finalization() {
                            self.store.set_content(
                                hit.element_id,
                                None,
                                hit.save_raw_text.then_some(raw_start..raw_start),
                                hit.save_text.then_some(text_start..text_start),
                            );
                        }
                    }
                    if let Some(behavior) = text_behavior
                        && text_was_active
                        && !behavior.suppressed
                        && open_text_tag_flags.is_break()
                    {
                        self.text_state.queue_separator(PendingSeparator::LineBreak);
                    }
                    self.matcher.close();
                } else {
                    for hit in &self.temp_state.save_hits {
                        if !hit.needs_close_finalization() {
                            continue;
                        }
                        let saved_index = self.temp_state.saved_elements.len();
                        self.temp_state.saved_elements.push(SavedElement::new(
                            hit.element_id,
                            hit.save_inner_html.then_some(self.position.reader_position),
                            hit.save_raw_text.then_some(raw_start),
                            hit.save_text.then_some(text_start),
                            text_edge_policy,
                        ));
                        self.open_elements.attach_saved(saved_index);
                    }
                    if CAPTURE {
                        self.raw_active_count += new_raw_count;
                        self.text_active_count += new_text_count;
                    }
                    if let Some(behavior) = text_behavior {
                        self.text_state
                            .after_open_element(behavior, self.position.element_depth);
                    }
                }
                finished = finished || self.finished();

                self.element.clear();
                if CAPTURE && self.capture_mode.captures_any() {
                    self.mark_active_source_start(reader.get_position());
                }
            }
            XHtmlTag::Close(closing_tag) => {
                // Opening tags flush before implied-close processing above.
                // Closing tags still need to flush their preceding text here.
                if CAPTURE && (self.raw_active_count > 0 || self.text_active_count > 0) {
                    self.flush_source_text(reader, tag_start_position);
                }
                if CAPTURE && self.capture_mode.captures_text() {
                    self.text_state.cancel_initial_newline();
                }
                finished = self.handle_close_tag::<CAPTURE>(closing_tag, reader) || finished;
                if CAPTURE && self.capture_mode.captures_any() {
                    self.mark_active_source_start(reader.get_position());
                }
            }
        }

        self.continue_after_tag::<CAPTURE>(finished, reader)
    }

    /// Skip the raw text of a `script`, `style` or similar element to its
    /// close tag, and close it. Out of line: most tags are not in raw text.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn next_after_raw_text<const CAPTURE: bool>(
        &mut self,
        reader: &mut Reader<'html>,
        close_tag: &'static str,
    ) -> bool {
        let source = reader.source();
        let Some(close_position) =
            self.indexer
                .find_raw_text_close(source, reader.get_position(), close_tag)
        else {
            reader.advance_to(source.len());
            self.drain_open_elements::<CAPTURE>(reader);
            return false;
        };
        reader.advance_to(close_position);

        // Consume an appropriate raw end tag here instead of delegating
        // to `XHtmlTag::from`: that parser intentionally keeps text after
        // `/` as part of the closing tag name, which would leave the raw
        // element open for a tolerated form such as `</style ignored>`.
        if CAPTURE && (self.raw_active_count > 0 || self.text_active_count > 0) {
            self.flush_source_text(reader, reader.get_position());
        }
        self.raw_text_close = None;

        self.position.reader_position = reader.get_position();
        reader.next_until(b'>');
        reader.skip();

        let closing_tag = &close_tag[2..];
        if CAPTURE && self.capture_mode.captures_text() {
            self.text_state.cancel_initial_newline();
        }
        let finished = self.handle_close_tag::<CAPTURE>(closing_tag, reader);
        if CAPTURE && self.capture_mode.captures_any() {
            self.mark_active_source_start(reader.get_position());
        }
        self.continue_after_tag::<CAPTURE>(finished, reader)
    }

    /// Whether another tag should be processed. At the end of the input
    /// (trailing whitespace included) elements still open are closed, so
    /// their saved content is finalized.
    #[inline(always)]
    fn continue_after_tag<const CAPTURE: bool>(
        &mut self,
        finished: bool,
        reader: &mut Reader<'html>,
    ) -> bool {
        if finished {
            crate::scah_trace!(
                self.store,
                TraceEvent::EarlyExit {
                    reader_position: reader.get_position(),
                }
            );
            return false;
        }
        if reader.eof() {
            reader.advance_to(reader.source().len());
            self.drain_open_elements::<CAPTURE>(reader);
            return false;
        }
        true
    }

    pub fn matches(mut self) -> Store<'html, 'query> {
        self.store.finish();
        self.store
    }

    pub fn trace_parse_started(
        &mut self,
        #[cfg_attr(not(any(debug_assertions, test)), allow(unused_variables))] html_len: usize,
        #[cfg_attr(not(any(debug_assertions, test)), allow(unused_variables))] query_count: usize,
    ) {
        crate::scah_trace!(
            self.store,
            TraceEvent::ParseStarted {
                html_len,
                query_count,
            }
        );
    }

    pub fn take_parse_error(&mut self) -> Option<ParseError> {
        self.parse_error.take()
    }

    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn record_parse_error(&mut self, err: ParseError) {
        if self.parse_error.is_none() {
            self.parse_error = Some(err);
        }
    }

    pub fn finish(mut self) -> Store<'html, 'query> {
        self.store.finish();
        crate::scah_trace!(
            self.store,
            TraceEvent::ParseFinished {
                element_count: self.store.len(),
                result_count: self.store.result_count(),
                attribute_count: self.store.attribute_count(),
                raw_text_len: self.store.text.raw_text.len(),
                text_len: self.store.text.text.len(),
            }
        );
        self.store
    }

    /// Close an element popped from the open-element stack. Returns whether
    /// parsing is finished.
    fn pop_open_element<const CAPTURE: bool>(
        &mut self,
        open_element: OpenElement<'html>,
        close_depth: DepthSize,
        reader: &Reader<'html>,
    ) -> bool {
        let saved_range = OpenElementStack::saved_range(&open_element);
        debug_assert_eq!(saved_range.end, self.temp_state.saved_elements.len());
        let (closing_raw_count, closing_text_count) =
            self.finalize_open_element::<CAPTURE>(&open_element, reader);
        if let Some(structural) = self.temp_state.structural.as_deref_mut() {
            structural.close();
        }
        self.temp_state.saved_elements.truncate(saved_range.start);
        if CAPTURE {
            debug_assert!(closing_raw_count <= self.raw_active_count);
            debug_assert!(closing_text_count <= self.text_active_count);
            self.raw_active_count = self.raw_active_count.saturating_sub(closing_raw_count);
            self.text_active_count = self.text_active_count.saturating_sub(closing_text_count);
        }
        if CAPTURE && self.capture_mode.captures_text() {
            let text_flags = open_element.text_flags();
            self.text_state.after_close_element(text_flags, close_depth);
            if self.text_active_count == 0 {
                self.text_state.discard_pending();
            } else if !text_flags.contains(TextElementFlags::SUPPRESSED) {
                if text_flags.contains(TextElementFlags::CELL) {
                    self.text_state.queue_cell_boundary();
                } else if let Some(separator) = text_flags.post_text_separator() {
                    self.text_state.queue_separator(separator);
                }
            }
        }
        self.position.element_depth = close_depth;
        self.matcher.close();
        self.finished()
    }

    /// Drain the implied-closes vector, finalizing each element, and restore
    /// the vector's capacity for reuse. Returns whether parsing is finished.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn drain_implied_closes<const CAPTURE: bool>(
        &mut self,
        reader: &Reader<'html>,
        implied_close_reason: Option<ImpliedCloseReason>,
        expected_tag: Option<&'html str>,
    ) -> bool {
        let mut elements = std::mem::take(&mut self.temp_state.implied_closes);
        let finished =
            self.pop_elements::<CAPTURE>(&mut elements, reader, implied_close_reason, expected_tag);
        self.temp_state.implied_closes = elements;
        finished
    }

    /// Apply a close tag: trace, pop from the open-element stack, and run
    /// the close-element path. Returns whether parsing is finished.
    fn handle_close_tag<const CAPTURE: bool>(
        &mut self,
        closing_tag: &'html str,
        reader: &Reader<'html>,
    ) -> bool {
        crate::scah_trace!(
            self.store,
            TraceEvent::CloseTag {
                tag: closing_tag,
                depth: self.position.element_depth,
                reader_position: self.position.reader_position,
            }
        );

        // Well-formed markup closes the element on top of the stack. Handling
        // that here duplicates `close_by_end_tag_into`'s fast path on purpose:
        // it keeps the common case off `temp_state.closing_elements` entirely.
        // The outcome is identical, because `pop_elements` suppresses the
        // implied-close trace when the popped name matches `expected_tag` and
        // derives the same `close_depth` for a single popped element.
        // Well-formed markup repeats the open tag's exact name, so try that
        // before resolving the close tag's id.
        if let Some(open_element) = self.open_elements.pop_exact_top(closing_tag) {
            let close_depth = self.open_elements.depth().saturating_add(1);
            return self.pop_open_element::<CAPTURE>(open_element, close_depth, reader);
        }
        let id = TagId::of(closing_tag);
        if let Some(open_element) = self.open_elements.pop_matching_top(closing_tag, id) {
            let close_depth = self.open_elements.depth().saturating_add(1);
            return self.pop_open_element::<CAPTURE>(open_element, close_depth, reader);
        }

        self.close_mismatched::<CAPTURE>(closing_tag, id, reader)
    }

    /// Apply a close tag that does not close the top element: it closes an
    /// element further down, or nothing.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn close_mismatched<const CAPTURE: bool>(
        &mut self,
        closing_tag: &'html str,
        id: TagId,
        reader: &Reader<'html>,
    ) -> bool {
        self.open_elements.close_by_end_tag_into(
            closing_tag,
            id,
            &mut self.temp_state.closing_elements,
        );
        let mut elements = std::mem::take(&mut self.temp_state.closing_elements);
        let finished = self.pop_elements::<CAPTURE>(
            &mut elements,
            reader,
            Some(ImpliedCloseReason::MismatchedEndTag),
            Some(closing_tag),
        );
        self.temp_state.closing_elements = elements;
        finished
    }

    /// Close a batch of elements popped together, innermost first.
    fn pop_elements<const CAPTURE: bool>(
        &mut self,
        elements: &mut Vec<OpenElement<'html>>,
        reader: &Reader<'html>,
        implied_close_reason: Option<ImpliedCloseReason>,
        expected_tag: Option<&'html str>,
    ) -> bool {
        let base_depth = self.open_elements.depth();
        let total = elements.len();
        let mut finished = false;

        for (index, open_element) in elements.drain(..).enumerate() {
            let close_depth = base_depth.saturating_add((total - index) as DepthSize);
            if implied_close_reason.is_some_and(|_| {
                expected_tag
                    .is_none_or(|expected| !open_element.name.eq_ignore_ascii_case(expected))
            }) {
                crate::scah_trace!(
                    self.store,
                    TraceEvent::ImpliedClose {
                        tag: open_element.name,
                        depth: close_depth,
                        reason: implied_close_reason.unwrap(),
                    }
                );
            }
            finished =
                self.pop_open_element::<CAPTURE>(open_element, close_depth, reader) || finished;
        }
        finished
    }

    fn finalize_open_element<const CAPTURE: bool>(
        &mut self,
        open_element: &OpenElement<'html>,
        reader: &Reader<'html>,
    ) -> (usize, usize) {
        let mut raw_count = 0;
        let mut text_count = 0;
        for saved_index in OpenElementStack::saved_range(open_element) {
            let saved = &self.temp_state.saved_elements[saved_index];
            let inner_html = saved
                .inner_html_start()
                .map(|start| reader.slice(start..self.position.reader_position));
            if !CAPTURE {
                self.store
                    .set_content(saved.element_id, inner_html, None, None);
                continue;
            }
            let raw_text = saved
                .raw_text_start()
                .map(|start| start..self.store.text.raw_text.len());
            let text = saved.text_start().map(|start| {
                let range = start..self.store.text.text.len();
                match saved.text_edge_policy() {
                    TextEdgePolicy::TrimCollapsedSeparators => {
                        trim_collapsed_range(&self.store.text.text, range)
                    }
                    TextEdgePolicy::Preserve => range,
                }
            });
            raw_count += usize::from(raw_text.is_some());
            text_count += usize::from(text.is_some());
            self.store
                .set_content(saved.element_id, inner_html, raw_text, text);
        }
        (raw_count, text_count)
    }

    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn drain_open_elements<const CAPTURE: bool>(&mut self, reader: &Reader<'html>) {
        if self.eof_drained {
            return;
        }

        if CAPTURE && (self.raw_active_count > 0 || self.text_active_count > 0) {
            self.flush_source_text(reader, reader.get_position());
        }
        self.position.reader_position = reader.get_position();
        self.open_elements
            .close_all_at_eof_into(&mut self.temp_state.implied_closes);
        self.drain_implied_closes::<CAPTURE>(reader, Some(ImpliedCloseReason::EofDrain), None);
        self.eof_drained = true;
    }
}
#[cfg(test)]
mod tests {

    /// Attributes other than `class` and `id` of every saved row, in row
    /// order.
    fn saved_attributes<'html, 'query: 'html>(
        store: &Store<'html, 'query>,
    ) -> Vec<Attribute<'html>> {
        store
            .elements()
            .flat_map(|element| element.attributes().into_iter().flatten())
            .collect()
    }

    use super::*;
    use crate::Attribute;
    use crate::store::ElementRef;
    use crate::{Query, Reader, Save, parse};
    use pretty_assertions::assert_eq;

    const BASIC_HTML: &str = r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#;

    #[test]
    fn stepped_sibling_parses_match_parse_results_for_all_and_first() {
        let html = "<main><h1></h1><p id='one'></p><p id='two'></p></main>";

        let all_queries = &[Query::all("h1 ~ p", Save::none()).unwrap().build()];
        let expected_all = parse(html, all_queries).unwrap();
        let mut all_parser = XHtmlParser::new(all_queries);
        let mut all_reader = Reader::new(html);
        while all_parser.next(&mut all_reader) {}
        let actual_all = all_parser.matches();
        assert_eq!(actual_all.get("h1 ~ p").unwrap().count(), 2);
        assert_eq!(
            actual_all.get("h1 ~ p").unwrap().count(),
            expected_all.get("h1 ~ p").unwrap().count()
        );

        let first_queries = &[Query::first("h1 ~ p", Save::none()).unwrap().build()];
        let expected_first = parse(html, first_queries).unwrap();
        let mut first_parser = XHtmlParser::new(first_queries);
        let mut first_reader = Reader::new(html);
        while first_parser.next(&mut first_reader) {}
        let actual_first = first_parser.matches();
        assert_eq!(actual_first.get("h1 ~ p").unwrap().count(), 1);
        assert_eq!(
            actual_first.get("h1 ~ p").unwrap().count(),
            expected_first.get("h1 ~ p").unwrap().count()
        );
    }

    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(BASIC_HTML);

        let queries = &[Query::all("p.indent > .bold", Save::none())
            .unwrap()
            .build()];

        let mut parser = XHtmlParser::new(queries);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        println!("{:?}", store);

        assert_eq!(store.get("p.indent > .bold").unwrap().count(), 1);
        let children = store.get("p.indent > .bold").unwrap();

        let children: Vec<ElementRef> = children.collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name(), "span");
        assert_eq!(children[0].id(), Some("name"));
        assert_eq!(children[0].class(), Some("bold"));
    }

    #[test]
    fn test_text_content() {
        let queries = &[Query::all("p.indent > .bold", Save::only_text())
            .unwrap()
            .build()];
        let store = parse(BASIC_HTML, queries).unwrap();
        let bold = store.get("p.indent > .bold").unwrap().next().unwrap();
        assert_eq!(bold.text(), Some("Zachary"));
    }

    #[test]
    fn test_text_content_buffer_is_empty_when_queries_do_not_request_text() {
        let html = "<div><a href='x'>Hello <b>World</b></a></div>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("a", Save::only_inner_html()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let anchor = store.get("a").unwrap().next().unwrap();
        assert_eq!(anchor.inner_html(), Some("Hello <b>World</b>"));
        assert_eq!(anchor.text(), None);
        assert!(store.text.text.as_bytes().is_empty());
    }

    #[test]
    fn unmatched_text_query_leaves_both_tapes_empty() {
        let html = "<main>outside &amp; text<div>more text</div></main>";
        let queries = &[Query::all(".missing", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);
        let mut reader = Reader::new(html);

        parser.run(&mut reader);

        assert!(parser.store.text.raw_text.as_bytes().is_empty());
        assert!(parser.store.text.text.as_bytes().is_empty());
        assert_eq!(parser.raw_active_count, 0);
        assert_eq!(parser.text_active_count, 0);
        #[cfg(feature = "bench-internals")]
        {
            assert_eq!(parser.text_state.path_stats.flush_calls, 0);
            assert_eq!(parser.text_state.path_stats.decoded_fragments, 0);
        }
    }

    #[test]
    fn sparse_text_query_captures_only_the_matching_subtree() {
        let html = concat!(
            "<main>outside &amp; text",
            "<div class='hit'>selected <b>&amp; nested</b></div>",
            "<section>trailing &amp; text</section></main>"
        );
        let queries = &[Query::all(".hit", Save::all()).unwrap().build()];
        let store = parse(html, queries).unwrap();
        let hit = store.get(".hit").unwrap().next().unwrap();

        assert_eq!(hit.raw_text(), Some("selected &amp; nested"));
        assert_eq!(hit.text(), Some("selected & nested"));
        assert_eq!(store.text.raw_text.as_bytes(), b"selected &amp; nested");
        assert_eq!(store.text.text.as_bytes(), b"selected & nested");
    }

    #[test]
    fn test_mixed_queries_keep_text_content_available_when_any_query_requests_it() {
        let html = "<div><a href='x'>Hello <b>World</b></a></div>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("a", Save::only_inner_html()).unwrap().build(),
            Query::all("b", Save::only_text()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let anchor = store.get("a").unwrap().next().unwrap();
        let bold = store.get("b").unwrap().next().unwrap();
        assert_eq!(anchor.inner_html(), Some("Hello <b>World</b>"));
        assert_eq!(anchor.text(), None);
        assert_eq!(bold.text(), Some("World"));
    }

    #[test]
    fn test_top_level_multi_selection() {
        let mut reader = Reader::new(BASIC_HTML);

        let queries = &[
            Query::all("p.indent > .bold", Save::none())
                .unwrap()
                .build(),
            Query::all(".indent #name", Save::none()).unwrap().build(),
        ];

        let mut parser = XHtmlParser::new(queries);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        while parser.next(&mut reader) {}
    }

    const MORE_ADVANCED_BASIC_HTML: &str = r#"
        <html>
            <h1>Hello World</h1>
            <main>
                <section>
                    <a href="https://hello.com">Hello</a>
                    <div>
                        <a href="https://world.com">World</a>
                    </div>
                </section>
            </main>

            <main>
                <section>
                    <a href="https://hello2.com">Hello2</a>

                    <div>
                        <a href="https://world2.com">World2</a>
                        <div>
                            <a href="https://world3.com">World3</a>
                        </div>
                    </div>
                </section>
            </main>
        </html>
        "#;

    #[test]
    #[ignore = "Known issue: Duplication of elements is not handled"]
    fn test_multi_selection() {
        let mut reader = Reader::new(MORE_ADVANCED_BASIC_HTML);
        let queries = Query::all("main > section", Save::all())
            .unwrap()
            .then(|section| {
                Ok([
                    section.all("> a[href]", Save::all())?,
                    section.all("div a", Save::all())?,
                ])
            })
            .unwrap();
        let queries = &[queries.build()];
        let mut parser = XHtmlParser::new(queries);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        println!("{:#?}", store);

        let sections: Vec<ElementRef> = store.get("main > section").unwrap().collect();
        assert_eq!(sections.len(), 2);

        // Section 1
        let s1 = sections[0];
        assert_eq!(s1.text(), Some("Hello World"));

        let s1_div_a: Vec<ElementRef> = s1.get("div a").unwrap().collect();
        assert_eq!(s1_div_a.len(), 1);
        assert_eq!(s1_div_a[0].text(), Some("World"));
        assert_eq!(
            s1_div_a[0].attributes().unwrap().next().unwrap().value,
            Some("https://world.com")
        );

        println!("{:#?}", s1);

        let s1_direct_a: Vec<ElementRef> = s1.get("> a[href]").unwrap().collect();
        assert_eq!(s1_direct_a.len(), 1);
        assert_eq!(s1_direct_a[0].text(), Some("Hello"));
        assert_eq!(
            s1_direct_a[0].attributes().unwrap().next().unwrap().value,
            Some("https://hello.com")
        );

        // Section 2
        let s2 = sections[1];
        assert_eq!(s2.text(), Some("Hello2 World2 World3"));

        let s2_div_a: Vec<ElementRef> = s2.get("div a").unwrap().collect();
        assert_eq!(s2_div_a.len(), 2, "World3 Element duplicated");
        assert_eq!(s2_div_a[0].text(), Some("World2"));
        assert_eq!(s2_div_a[1].text(), Some("World3"));

        let s2_direct_a: Vec<ElementRef> = s2.get("> a[href]").unwrap().collect();
        assert_eq!(s2_direct_a.len(), 1);
        assert_eq!(s2_direct_a[0].text(), Some("Hello2"));
    }

    const BASIC_HTML_WITH_SCRIPT: &str = r#"
        <html>
            <h1>Hello World</h1>

            <script>
                let x = 123132.2;
                let y = "<div>" + "Hello" + "</" + "div>";
            </script>
        </html>
        "#;

    #[test]
    fn test_script_tag_with_html_like_content() {
        let mut reader = Reader::new(BASIC_HTML_WITH_SCRIPT);

        let queries = &[Query::all("div", Save::none()).unwrap().build()];

        let mut parser = XHtmlParser::new(queries);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        // It should NOT find any div
        if let Some(div_idx) = store.get("div") {
            assert_eq!(div_idx.count(), 0);
        }
    }

    #[test]
    fn full_index_skips_false_events_inside_long_raw_text() {
        let html = format!(
            "<script>{}<span>fake</span></script><span>real</span>",
            "const x = 1;".repeat(2_000)
        );
        let queries = &[Query::all("span", Save::name_only()).unwrap().build()];

        let store = parse(&html, queries).unwrap();
        let matches = store.get("span").unwrap().collect::<Vec<_>>();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].inner_html(), None);
    }

    #[test]
    fn full_index_finds_raw_close_after_tag_like_quote_inside_script() {
        let html = format!(
            "<script>{}const s = \"<div data='unterminated\";</script><span>real</span>",
            "x".repeat(20_000)
        );
        let queries = &[Query::all("span", Save::name_only()).unwrap().build()];

        let store = parse(&html, queries).unwrap();

        assert_eq!(store.get("span").unwrap().count(), 1);
    }

    #[test]
    fn full_index_uses_earliest_literal_raw_close() {
        for (open, close) in [("script", "script"), ("style", "style")] {
            let html = format!(
                "<{open}>{}<div data=\"</{close}><span>first</span>\"></{close}><span>second</span>",
                "x".repeat(20_000)
            );
            let queries = &[Query::all("span", Save::name_only()).unwrap().build()];

            let store = parse(&html, queries).unwrap();

            assert_eq!(store.get("span").unwrap().count(), 2, "raw tag {open}");
        }
    }

    const BASIC_HTML_WITH_SELF_CLOSING_TAG: &str = r#"
        <html>
            <h1>Hello World</h1>
            <form action="/my-handling-form-page" method="post">
                <p>
                    <label for="name">Name:</label>
                    <input type="text" id="name" name="user_name" />
                </p>
                <p>
                    <label for="mail">Email:</label>
                    <input type="email" id="mail" name="user_email" />
                </p>
                <p>
                    <label for="msg">Message:</label>
                    <textarea id="msg" name="user_message"></textarea>
                </p>
            </form>
        </html>
        "#;

    #[test]
    fn test_self_closing_tags() {
        let mut reader = Reader::new(BASIC_HTML_WITH_SELF_CLOSING_TAG);
        let queries = &[Query::all("form > p > input", Save::none())
            .unwrap()
            .build()];

        let mut parser = XHtmlParser::new(queries);

        println!("{:?}", queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        let inputs: Vec<ElementRef> = store.get("form > p > input").unwrap().collect();
        assert_eq!(inputs.len(), 2);

        assert_eq!(inputs[0].name(), "input");
        assert_eq!(inputs[0].id(), Some("name"));
        assert_eq!(inputs[0].attributes().unwrap().next().unwrap().key, "type");
        assert_eq!(
            inputs[0].attributes().unwrap().next().unwrap().value,
            Some("text")
        );

        assert_eq!(inputs[1].name(), "input");
        assert_eq!(inputs[1].id(), Some("mail"));
        assert_eq!(inputs[1].attributes().unwrap().next().unwrap().key, "type");
        assert_eq!(
            inputs[1].attributes().unwrap().next().unwrap().value,
            Some("email")
        );
    }

    #[test]
    fn test_self_closing_tags_with_content_query() {
        let mut reader = Reader::new(BASIC_HTML_WITH_SELF_CLOSING_TAG);

        let queries = &[Query::all("form > p > input", Save::all()).unwrap().build()];

        let mut parser = XHtmlParser::new(queries);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        let inputs: Vec<ElementRef> = store.get("form > p > input").unwrap().collect();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].text(), Some(""));
        assert_eq!(inputs[0].inner_html(), None);

        assert_eq!(inputs[1].text(), Some(""));
        assert_eq!(inputs[1].inner_html(), None);
    }

    const BASIC_ANCHOR_LIST: &str = r#"
        <a>Hello 1</a>
        <a>Hello 2</a>
        <a>Hello 3</a>
        "#;

    #[test]
    fn test_anchor_list_selection() {
        let mut reader = Reader::new(BASIC_ANCHOR_LIST);

        let queries = &[Query::all("a", Save::all()).unwrap().build()];

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        let anchors: Vec<ElementRef> = store.get("a").unwrap().collect();
        assert_eq!(anchors.len(), 3);

        assert_eq!(anchors[0].text(), Some("Hello 1"));
        assert_eq!(anchors[1].text(), Some("Hello 2"));
        assert_eq!(anchors[2].text(), Some("Hello 3"));
    }

    const POSTS: &str = r#"<div class="article"><a href="/post/0"><b>Post</b> &lt;0&gt;</a></div><div class="article"><a href="/post/1"><b>Post</b> &lt;1&gt;</a></div>"#;

    #[test]
    fn test_first_anchor_in_list_selection() {
        let mut reader = Reader::new(POSTS);

        let queries = &[Query::first("div.article a", Save::all()).unwrap().build()];

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        let anchor = store.get("div.article a").unwrap().next().unwrap();

        assert_eq!(anchor.name(), "a");
        assert_eq!(
            anchor.attributes().unwrap().next().unwrap().value,
            Some("/post/0")
        );
        assert_eq!(anchor.inner_html(), Some("<b>Post</b> &lt;0&gt;"));
        assert_eq!(anchor.text(), Some("Post <0>"));
    }

    const PYTHON_TEST_HTML: &str = r#"
    <span class="hello" id="world" hello="world">
        Hello <a href="https://www.example.com">World</a>
    </span>
    <p class="example_class" id="example_id" hello="example">
        My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
    </p>
    "#;

    #[test]
    fn test_python_test_html() {
        let mut reader = Reader::new(PYTHON_TEST_HTML);

        let queries = &[Query::all("#world", Save::all())
            .unwrap()
            .all("a", Save::all())
            .unwrap()
            .build()];

        // assert_eq!(queries, &[Query {
        //     queries: vec![].into_boxed_slice(),
        //     states: vec![].into_boxed_slice(),
        //     exit_at_section_end: None,
        // }]);

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        assert_eq!(
            saved_attributes(&store),
            vec![
                Attribute {
                    key: "hello",
                    value: Some("world")
                },
                Attribute {
                    key: "href",
                    value: Some("https://www.example.com")
                },
            ]
        );

        let worlds: Vec<ElementRef> = store.get("#world").unwrap().collect();
        assert_eq!(worlds.len(), 1);

        let span = worlds[0];
        assert_eq!(span.name(), "span");
        assert_eq!(span.class(), Some("hello"));
        assert_eq!(span.id(), Some("world"));
        assert_eq!(
            span.attributes().unwrap().collect::<Vec<_>>(),
            &[Attribute {
                key: "hello",
                value: Some("world")
            },]
        );
        assert_eq!(
            span.inner_html(),
            Some(
                r#"
        Hello <a href="https://www.example.com">World</a>
    "#
            )
        );
        assert!(span.text().is_some());

        let anchors: Vec<ElementRef> = span.get("a").unwrap().collect();
        assert_eq!(anchors.len(), 1);

        let a = anchors[0];
        assert_eq!(a.name(), "a");
        assert_eq!(a.class(), None);
        assert_eq!(a.id(), None);
        assert_eq!(
            a.attributes().unwrap().collect::<Vec<_>>(),
            &[Attribute {
                key: "href",
                value: Some("https://www.example.com")
            },]
        );
        assert_eq!(a.inner_html(), Some("World"));
        assert!(a.text().is_some());
    }

    #[test]
    fn test_first_anchor_tag_from_bench() {
        fn generate_html(count: usize) -> String {
            let mut html = String::with_capacity(count * 100);
            html.push_str("<html><body><div id='content'>");
            for i in 0..count {
                // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
                html.push_str(&format!(
                    r#"<div class="article"><a href="/post/{}"><b>Post</b> &lt;{}&gt;</a></div>"#,
                    i, i
                ));
            }
            html.push_str("</div></body></html>");
            html
        }

        let html = generate_html(100);
        let mut reader = Reader::from_bytes(html.as_bytes());

        let query = Query::first("a", Save::all()).unwrap().build();
        let queries = &[query];
        assert!(Program::compile(queries).features().all_roots_first);

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        let element = store.get("a").unwrap().next().unwrap();

        assert_eq!(
            saved_attributes(&store),
            vec![Attribute {
                key: "href",
                value: Some("/post/0"),
            }]
        );

        assert_eq!(element.inner_html(), Some("<b>Post</b> &lt;0&gt;"));
        assert_eq!(element.text(), Some("Post <0>"));
    }

    #[test]
    fn test_implicit_p_close_finalizes_content() {
        let html = "<div><p>Hello<div>World</div></div>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("p", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let p = store.get("p").unwrap().next().unwrap();
        assert_eq!(p.inner_html(), Some("Hello"));
        assert_eq!(p.text(), Some("Hello"));
    }

    #[test]
    fn implied_close_updates_query_frontier_before_open_tag_preflight() {
        let html = "<p><a>one</a><p><a>two</a>";
        let queries = &[Query::all("p a", Save::name_only()).unwrap().build()];

        let store = parse(html, queries).unwrap();

        assert_eq!(store.get("p a").unwrap().count(), 2);
    }

    #[test]
    fn test_misnested_close_finalizes_bubbled_elements() {
        let html = "<div><span>Hello</div>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("div", Save::all()).unwrap().build(),
            Query::all("span", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let div = store.get("div").unwrap().next().unwrap();
        let span = store.get("span").unwrap().next().unwrap();

        assert_eq!(span.inner_html(), Some("Hello"));
        assert_eq!(span.text(), Some("Hello"));
        assert_eq!(div.inner_html(), Some("<span>Hello"));
        assert_eq!(div.text(), Some("Hello"));
    }

    #[test]
    fn test_stray_close_tag_is_ignored() {
        let html = "<div><span>Hello</bogus></span></div>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("div span", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let span = store.get("div span").unwrap().next().unwrap();
        assert_eq!(span.text(), Some("Hello"));
        assert_eq!(span.inner_html(), Some("Hello</bogus>"));
    }

    #[test]
    fn test_eof_drain_finalizes_open_elements() {
        let html = "<section><a href='x'>Link";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("section", Save::all()).unwrap().build(),
            Query::all("a", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let section = store.get("section").unwrap().next().unwrap();
        let a = store.get("a").unwrap().next().unwrap();

        assert_eq!(a.inner_html(), Some("Link"));
        assert_eq!(a.text(), Some("Link"));
        assert_eq!(section.inner_html(), Some("<a href='x'>Link"));
        assert_eq!(section.text(), Some("Link"));
    }

    #[test]
    fn test_li_auto_close_on_next_li() {
        let html = "<ul><li>One<li>Two</ul>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("li", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let items: Vec<ElementRef> = store.get("li").unwrap().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text(), Some("One"));
        assert_eq!(items[0].inner_html(), Some("One"));
        assert_eq!(items[1].text(), Some("Two"));
        assert_eq!(items[1].inner_html(), Some("Two"));
    }

    #[test]
    fn implied_close_preflight_matches_reactivated_attribute_cursor() {
        let html = "<ul><li data-x='one'>One<li data-x='two'>Two</ul>";
        let queries = &[Query::all("li[data-x]", Save::name_only()).unwrap().build()];

        let store = parse(html, queries).unwrap();
        assert_eq!(store.get("li[data-x]").unwrap().count(), 2);
    }

    #[test]
    fn test_dt_dd_auto_close_sequence() {
        let html = "<dl><dt>Term<dd>Def<dt>Next</dl>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("dt", Save::all()).unwrap().build(),
            Query::all("dd", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let dts: Vec<ElementRef> = store.get("dt").unwrap().collect();
        let dds: Vec<ElementRef> = store.get("dd").unwrap().collect();

        assert_eq!(dts.len(), 2);
        assert_eq!(dds.len(), 1);
        assert_eq!(dts[0].text(), Some("Term"));
        assert_eq!(dds[0].text(), Some("Def"));
        assert_eq!(dts[1].text(), Some("Next"));
    }

    #[test]
    fn test_option_auto_close_on_next_option() {
        let html = "<select><option>One<option>Two</select>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("option", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let options: Vec<ElementRef> = store.get("option").unwrap().collect();
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].text(), Some("One"));
        assert_eq!(options[1].text(), Some("Two"));
    }

    #[test]
    fn test_optgroup_closes_previous_option_and_optgroup() {
        let html = "<select><optgroup><option>One<optgroup><option>Two</select>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("optgroup", Save::all()).unwrap().build(),
            Query::all("option", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let optgroups: Vec<ElementRef> = store.get("optgroup").unwrap().collect();
        let options: Vec<ElementRef> = store.get("option").unwrap().collect();

        assert_eq!(optgroups.len(), 2);
        assert_eq!(options.len(), 2);
        assert_eq!(optgroups[0].text(), Some("One"));
        assert_eq!(optgroups[1].text(), Some("Two"));
        assert_eq!(options[0].text(), Some("One"));
        assert_eq!(options[1].text(), Some("Two"));
    }

    #[test]
    fn test_td_auto_close_on_next_td() {
        let html = "<table><tr><td>One<td>Two</tr></table>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("td", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let cells: Vec<ElementRef> = store.get("td").unwrap().collect();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].text(), Some("One"));
        assert_eq!(cells[1].text(), Some("Two"));
    }

    #[test]
    fn test_multiple_queries_attach_to_same_open_element() {
        let html = "<div class='x'>Hello</div>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("div", Save::all()).unwrap().build(),
            Query::all(".x", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let div = store.get("div").unwrap().next().unwrap();
        let class_match = store.get(".x").unwrap().next().unwrap();

        assert_eq!(div.inner_html(), Some("Hello"));
        assert_eq!(div.text(), Some("Hello"));
        assert_eq!(class_match.inner_html(), Some("Hello"));
        assert_eq!(class_match.text(), Some("Hello"));
    }

    #[test]
    fn test_descendant_and_child_queries_remain_stable_on_malformed_html() {
        let html = "<div><span>One</div><div><span>Two</span></div>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("div span", Save::all()).unwrap().build(),
            Query::all("div > span", Save::all()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        assert_eq!(store.get("div span").unwrap().count(), 2);
        assert_eq!(store.get("div > span").unwrap().count(), 2);
    }

    #[test]
    fn attribute_query_ignores_incomplete_open_delimiters_at_eof() {
        let queries = &[Query::all("[data-x]", Save::all()).unwrap().build()];

        for html in ["<", "hello <", "<<<", "<div data-x>text<"] {
            let store = parse(html, queries).unwrap();
            let expected = usize::from(html.starts_with("<div "));
            assert_eq!(
                store.get("[data-x]").map_or(0, Iterator::count),
                expected,
                "{html:?}"
            );
        }
    }

    #[test]
    fn comment_does_not_drop_following_text_content() {
        let queries = &[Query::all("div", Save::only_text()).unwrap().build()];
        let store = parse("<div>abc<!--c-->def</div>", queries).unwrap();
        let div = store.get("div").unwrap().next().unwrap();

        assert_eq!(div.text(), Some("abcdef"));
    }

    #[test]
    fn full_index_trailing_bare_delimiter_still_drains_open_elements() {
        let unit = format!("{}<span></span>", "x".repeat(900));
        let html = format!("<div>{}<", unit.repeat(200));
        let queries = &[Query::all("div", Save::all()).unwrap().build()];

        let store = parse(&html, queries).unwrap();
        let div = store.get("div").unwrap().next().unwrap();

        assert_eq!(div.inner_html(), Some(&html["<div>".len()..]));
    }

    #[test]
    fn full_index_ignores_a_trailing_bare_open_delimiter() {
        let unit = format!("<p data-x='1'>a</p>{}", "y".repeat(400));
        let html = format!("{}<", unit.repeat(512));
        let queries = &[Query::all("[data-x]", Save::name_only()).unwrap().build()];

        let store = parse(&html, queries).unwrap();

        assert_eq!(store.get("[data-x]").unwrap().count(), 512);
    }

    #[test]
    fn test_text_before_first_tag_does_not_break_text_content() {
        let html = "intro<div>Hello</div>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("div", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let div = store.get("div").unwrap().next().unwrap();
        assert_eq!(div.text(), Some("Hello"));
    }

    #[test]
    fn save_none_does_not_accumulate_text_content() {
        let queries = &[Query::all("a", Save::none()).unwrap().build()];
        let store = parse("<div><a>Hello <b>World</b></a></div>", queries).unwrap();

        let anchor = store.get("a").unwrap().next().unwrap();
        assert_eq!(anchor.text(), None);
        assert_eq!(store.text.text.len(), 0);
    }

    #[test]
    fn mixed_save_queries_keep_text_content_for_text_query() {
        let queries = &[
            Query::all("a", Save::none()).unwrap().build(),
            Query::all("b", Save::only_text()).unwrap().build(),
        ];
        let store = parse("<a>Hello <b>World</b></a>", queries).unwrap();

        let anchor = store.get("a").unwrap().next().unwrap();
        let bold = store.get("b").unwrap().next().unwrap();
        assert_eq!(anchor.text(), None);
        assert_eq!(bold.text(), Some("World"));
    }

    const SINGLE_PRODUCT_HTML: &str = r#"
    <section id="products">
        <div class="product">
            <h1>Product #1</h1>
            <img src="https://example.com/p1.png"/>
            <p>
                Hello World for Product #1
            </p>
        </div>
    </section>
    "#;

    #[test]
    fn test_single_product_listing_html() {
        let mut reader = Reader::new(SINGLE_PRODUCT_HTML);

        let queries = &[Query::all("#products", Save::all())
            .unwrap()
            .all(".product", Save::all())
            .unwrap()
            .then(|p| {
                Ok([
                    p.first("h1", Save::all())?,
                    p.first("img", Save::none())?,
                    p.first("p", Save::all())?,
                ])
            })
            .unwrap()
            .build()];

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        println!("Store: {:#?}", store);

        assert_eq!(store.elements().len(), 5);

        assert_eq!(
            saved_attributes(&store),
            vec![Attribute {
                key: "src",
                value: Some("https://example.com/p1.png")
            }]
        );

        let products_sections: Vec<ElementRef> = store.get("#products").unwrap().collect();
        assert_eq!(products_sections.len(), 1);

        let section = products_sections[0];
        assert_eq!(section.name(), "section");
        assert_eq!(section.id(), Some("products"));
        assert!(section.inner_html().is_some());
        assert!(section.text().is_some());

        let products: Vec<ElementRef> = section.get(".product").unwrap().collect();
        assert_eq!(products.len(), 1);

        let product = products[0];
        assert_eq!(product.name(), "div");
        assert_eq!(product.class(), Some("product"));
        assert!(product.inner_html().is_some());
        assert!(product.text().is_some());

        let h1 = product.get("h1").unwrap().next().unwrap();
        assert_eq!(h1.name(), "h1");
        assert_eq!(h1.inner_html(), Some("Product #1"));
        assert!(h1.text().is_some());

        let img = product.get("img").unwrap().next().unwrap();
        assert_eq!(img.name(), "img");
        assert!(img.attributes().is_some());

        let p = product.get("p").unwrap().next().unwrap();
        assert_eq!(p.name(), "p");
        assert!(p.inner_html().is_some());
        assert!(p.text().is_some());
    }

    const PRODUCT_HTML: &str = r#"
    <section id="products">
        <div class="product">
            <h1>Product #1</h1>
            <img src="https://example.com/p1.png"/>
            <p>
                Hello World for Product #1
            </p>
        </div>
        
        <div class="product">
            <h1>Product #2</h1>
            <img src="https://example.com/p2.png"/>
            <p>
                Hello World for Product #2
            </p>
        </div>
    </section>
    "#;

    #[test]
    fn test_product_listing_html() {
        let mut reader = Reader::new(PRODUCT_HTML);

        let queries = &[Query::all("#products", Save::all())
            .unwrap()
            .all(".product", Save::all())
            .unwrap()
            .then(|p| {
                Ok([
                    p.first("h1", Save::all())?,
                    p.first("img", Save::none())?,
                    p.first("p", Save::all())?,
                ])
            })
            .unwrap()
            .build()];

        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        println!("Store: {:#?}", store);

        assert_eq!(store.elements().len(), 9);

        assert_eq!(
            saved_attributes(&store),
            vec![
                Attribute {
                    key: "src",
                    value: Some("https://example.com/p1.png")
                },
                Attribute {
                    key: "src",
                    value: Some("https://example.com/p2.png")
                },
            ]
        );

        let products_sections: Vec<ElementRef> = store.get("#products").unwrap().collect();
        assert_eq!(products_sections.len(), 1);

        let section = products_sections[0];
        assert_eq!(section.name(), "section");
        assert_eq!(section.id(), Some("products"));
        assert!(section.inner_html().is_some());
        assert!(section.text().is_some());

        let products: Vec<ElementRef> = section.get(".product").unwrap().collect();
        assert_eq!(products.len(), 2);

        // Product 1
        let p1 = products[0];
        assert_eq!(p1.name(), "div");
        assert_eq!(p1.class(), Some("product"));
        assert!(p1.inner_html().is_some());
        assert!(p1.text().is_some());

        let p1_h1 = p1.get("h1").unwrap().next().unwrap();
        assert_eq!(p1_h1.name(), "h1");
        assert_eq!(p1_h1.inner_html(), Some("Product #1"));
        assert!(p1_h1.text().is_some());

        let p1_img = p1.get("img").unwrap().next().unwrap();
        assert_eq!(p1_img.name(), "img");
        assert!(p1_img.attributes().is_some());

        let p1_p = p1.get("p").unwrap().next().unwrap();
        assert_eq!(p1_p.name(), "p");
        assert!(p1_p.inner_html().is_some());
        assert!(p1_p.text().is_some());

        // Product 2
        let p2 = products[1];
        assert_eq!(p2.name(), "div");
        assert_eq!(p2.class(), Some("product"));
        assert!(p2.inner_html().is_some());
        assert!(p2.text().is_some());

        let p2_h1 = p2.get("h1").unwrap().next().unwrap();
        assert_eq!(p2_h1.name(), "h1");
        assert!(p2_h1.inner_html().is_some());
        assert!(p2_h1.text().is_some());

        let p2_img = p2.get("img").unwrap().next().unwrap();
        assert_eq!(p2_img.name(), "img");
        assert!(p2_img.attributes().is_some());

        let p2_p = p2.get("p").unwrap().next().unwrap();
        assert_eq!(p2_p.name(), "p");
        assert!(p2_p.inner_html().is_some());
        assert!(p2_p.text().is_some());
    }

    // --- parse() Result tests ---

    #[test]
    fn empty_query_list_returns_error() {
        let html = "<main><a href='x'>x</a></main>";
        let queries: Vec<Query> = Vec::new();

        let result = parse(html, &queries);

        assert!(matches!(result, Err(crate::ParseError::EmptyQueries)));
    }

    #[test]
    fn non_empty_query_list_still_parses() {
        let html = "<main><a href='x'>x</a></main>";
        let queries = &[Query::all("a", Save::all())
            .expect("valid selector")
            .build()];

        let store = parse(html, queries).expect("parse succeeds");

        assert_eq!(store.get("a").unwrap().count(), 1);
    }

    #[test]
    fn tag_only_prefixes_skip_attributes_but_saved_elements_keep_them() {
        let html = concat!(
            "<main data-unused='root'>",
            "<section data-unused='middle'>",
            "<a href='/kept' rel='next'>link</a>",
            "</section></main>"
        );
        let queries = &[Query::all("main a", Save::none()).unwrap().build()];
        let mut reader = Reader::new(html);
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        // `main` is a tag-only, non-save transition and `section` cannot
        // match either active name. Only the terminal `a` needs attributes,
        // both for saving and for preserving the public result contract.
        assert_eq!(parser.attribute_parse_count, 1);
        let store = parser.matches();
        let anchor = store.get("main a").unwrap().next().unwrap();
        assert_eq!(anchor.attribute("href"), Some("/kept"));
        assert_eq!(anchor.attribute("rel"), Some("next"));
    }

    #[test]
    fn structural_queries_preserve_selective_attribute_parsing() {
        let html = concat!(
            "<main data-unused='root'>",
            "<ul data-unused='list'><li data-unused='item'></li></ul>",
            "<a href='/kept' data-unused='link'></a>",
            "</main>"
        );
        let queries = [
            Query::all("li:first-child", Save::none()).unwrap().build(),
            Query::all("a[href]", Save::none()).unwrap().build(),
        ];
        let mut reader = Reader::new(html);
        let mut parser = XHtmlParser::new(&queries);

        while parser.next(&mut reader) {}

        // The structural candidate and the href candidate need attributes.
        // Unrelated ancestors remain on the name-only path.
        assert_eq!(parser.attribute_parse_count, 2);
    }

    #[test]
    fn attribute_selectors_parse_each_name_viable_candidate() {
        let html = "<main><div class='miss'></div><div class='hit'></div></main>";
        let queries = &[Query::all("div.hit", Save::none()).unwrap().build()];
        let mut reader = Reader::new(html);
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        assert_eq!(parser.attribute_parse_count, 2);
        assert_eq!(parser.matches().get("div.hit").unwrap().count(), 1);
    }

    #[test]
    fn normalized_text_requests_only_hidden_on_unmatched_tags() {
        let html = concat!(
            "<div data-a='1' data-b='2'></div>",
            "<span hidden data-c='3' data-d='4'></span>"
        );
        let queries = &[
            Query::all("article", Save::only_text().without_attributes())
                .unwrap()
                .build(),
        ];
        let mut reader = Reader::new(html);
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        assert_eq!(parser.attribute_parse_count, 2);
        assert_eq!(parser.selected_attribute_count, 1);
        assert!(parser.matches().is_empty());
    }

    #[test]
    fn empty_opening_tags_do_not_reach_attribute_preflight() {
        let html = "<>< > <<>><p data-x='hit'></p>";
        let queries = &[Query::all("[data-x]", Save::all()).unwrap().build()];

        let store = parse(html, queries).expect("parse succeeds");

        assert_eq!(store.get("[data-x]").unwrap().count(), 1);
    }

    #[test]
    fn sibling_queries_leave_no_frames_after_eof_drain() {
        let html = "<main><h1></h1>";
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("h1 + p", Save::none()).unwrap().build(),
            Query::all("h1 ~ p", Save::none()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}
        assert_eq!(parser.matcher.depth(), 0, "EOF must close every frame");
    }

    #[test]
    fn void_sibling_source_matches_the_next_element() {
        let html = "<main><br><p id='hit'></p></main>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("br + p", Save::none()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        // Open <main>
        assert!(parser.next(&mut reader));

        // Void <br> opens and closes at once, so it is <p>'s previous sibling.
        assert!(parser.next(&mut reader));
        assert_eq!(parser.matcher.depth(), 1);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let hits: Vec<_> = store
            .get("br + p")
            .unwrap()
            .map(|element| element.id())
            .collect();
        assert_eq!(hits, [Some("hit")]);
    }

    #[test]
    fn chained_void_sibling_matches() {
        // div + br + p: the void <br> is both a match and the previous sibling.
        let html = "<main><div></div><br><p id='hit'></p></main>";
        let mut reader = Reader::new(html);
        let queries = &[Query::all("div + br + p", Save::none()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let hits: Vec<_> = store
            .get("div + br + p")
            .unwrap()
            .map(|element| element.id())
            .collect();
        assert_eq!(hits, [Some("hit")]);
    }

    #[test]
    fn same_batch_closes_do_not_leak_siblings_out_of_their_parent() {
        // Closing </main> pops section then div then main. Their sibling state
        // dies with their parents, so the later <p> matches nothing.
        let html = r#"
        <main>
          <div>
            <section>
        </main>
        <p id="outside-miss"></p>
        "#;
        let mut reader = Reader::new(html);
        let queries = &[
            Query::all("section + p", Save::none()).unwrap().build(),
            Query::all("div ~ p", Save::none()).unwrap().build(),
        ];
        let mut parser = XHtmlParser::new(queries);

        while parser.next(&mut reader) {}
        let store = parser.matches();
        assert_eq!(
            store
                .get("section + p")
                .map(|iter| iter.count())
                .unwrap_or(0),
            0
        );
        assert_eq!(
            store.get("div ~ p").map(|iter| iter.count()).unwrap_or(0),
            0
        );
    }

    #[test]
    #[should_panic(expected = "a parser reads one document")]
    fn stepping_a_second_document_after_saving_panics() {
        let first = "<p>one</p>";
        let second = String::from("<p>two</p>");
        let queries = [Query::all("p", Save::none()).unwrap().build()];
        let mut parser = XHtmlParser::new(&queries);
        let mut reader = Reader::new(first);
        while parser.next(&mut reader) {}
        let mut reader = Reader::new(&second);
        parser.next(&mut reader);
    }

    #[test]
    fn utf8_byte_readers_parse_like_string_readers() {
        let html = "<p>é</p>";
        let queries = [Query::all("p", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(&queries);
        let mut reader = Reader::from_bytes(html.as_bytes());
        while parser.next(&mut reader) {}
        let store = parser.matches();
        let p = store.get("p").unwrap().next().unwrap();
        assert_eq!(p.inner_html(), Some("é"));
    }

    #[test]
    #[should_panic(expected = "a parser reads UTF-8 HTML")]
    fn non_utf8_byte_readers_panic_before_binding() {
        let queries = [Query::all("p", Save::none()).unwrap().build()];
        let mut parser = XHtmlParser::new(&queries);
        let mut reader = Reader::from_bytes(b"\xff<p>");
        parser.next(&mut reader);
    }

    #[test]
    #[should_panic(expected = "a parser reads one document")]
    fn stepping_a_second_document_before_saving_panics() {
        // Nothing is saved after `<div>`, but the matcher already holds the
        // open `div`, so a `<p>` from another document must not match.
        let first = "<div><p>one</p></div>";
        let second = String::from("<p>two</p>");
        let queries = [Query::all("div p", Save::none()).unwrap().build()];
        let mut parser = XHtmlParser::new(&queries);
        let mut reader = Reader::new(first);
        parser.next(&mut reader);
        assert!(parser.store.is_empty());
        let mut reader = Reader::new(&second);
        parser.next(&mut reader);
    }
}
