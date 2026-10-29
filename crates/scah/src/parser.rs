//! The parse loop: one pass over the document, one tag at a time.
//!
//! ```text
//! next_tag   the next open or close tag; comments are skipped    (lexer)
//! open_tag   close the elements the tag implies                  (tree)
//!            select the candidate steps for its name             (engine)
//!            read only the attributes they, or capture, need     (lexer)
//!            push it, match it, save its matches                 (tree, engine, store)
//!            note where the saved elements' content starts       (capture)
//! end_tag    pop the element the tag names, and any above it     (tree)
//!            finish its saved content, pop its match state       (capture, engine)
//! ```
//!
//! Parsing stops at the end of the input, or as soon as every query is a
//! satisfied `first()`.

use crate::ParseError;
use crate::capture::{Capture, TextElementFlags};
use crate::debug::ImpliedCloseReason;
#[cfg(any(debug_assertions, test))]
use crate::debug::{TraceEvent, TraceStore};
use crate::engine::matcher::{AnyMatcher, SaveHit};
use crate::engine::structural::StructuralState;
use crate::output::Output;
use crate::{CapacityOptions, Program, QuerySpec, Store};
use scah_html::{
    Attribute, AttributeInterest, AutoTagIndexer, ClassifiedTag, DepthSize, IndexingMode,
    MAX_ELEMENT_DEPTH, OpenElement, OpenElementStack, OpenTagStart, Reader, TagEvent, TagFlags,
    TagId, TagIndexer, TagKind, XHtmlElement,
};
use scah_store::StoreBuilder;
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
    /// Elements popped together, innermost first, waiting to be closed.
    closing: Vec<OpenElement<'html>>,
    /// Attributes of the open tag being parsed, cleared for each tag.
    attributes: Vec<Attribute<'html>>,
    save_hits: Vec<SaveHit>,
    attribute_interest: AttributeInterest<'query>,
    structural: Option<Box<StructuralState<'html, 'query>>>,
}

pub struct XHtmlParser<'html, 'query> {
    position: DocumentPosition,
    matcher: AnyMatcher<'query>,
    out: Output<'html, 'query>,
    element: crate::XHtmlElement<'html>,
    open_elements: OpenElementStack<'html>,
    temp_state: ParserTempState<'html, 'query>,
    capture: Capture,
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
        let parse_attributes = features.parses_attributes || requirements.text;
        let indexing_mode = indexing_mode.unwrap_or(if features.all_roots_first {
            IndexingMode::Rolling
        } else {
            IndexingMode::Adaptive
        });
        let options = CapacityOptions {
            reserve_raw_text: requirements.raw_text,
            reserve_text: requirements.text,
            ..CapacityOptions::default()
        };
        let store = capacity.map_or_else(StoreBuilder::default, |capacity| {
            StoreBuilder::with_capacity_requirements(
                capacity,
                options,
                features.stores_attributes,
                false,
            )
        });
        // The trace reserves an event per reserved row, up to a limit.
        let trace_capacity = capacity.map_or(0, |capacity| {
            (capacity / options.element_bytes_per_slot.max(1)).min(options.trace_capacity_limit)
        });
        let structural = features
            .has_structural
            .then(|| Box::new(StructuralState::new(&program)));
        let attribute_interest = AttributeInterest::new(program.attribute_names());
        let out = Output::new(store, &program, trace_capacity);

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
            capture: Capture::new(requirements),
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
            out,
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
        if self.capture.captures_any() {
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
        if self.capture.captures_any() {
            while self.next_mode::<true>(reader) {}
        } else {
            self.run_without_text_capture(reader);
        }
    }

    pub(crate) fn run_without_text_capture(&mut self, reader: &mut Reader<'html>) {
        debug_assert!(!self.capture.captures_any());
        if self.parse_error.is_some() {
            return;
        }
        self.bind_source(reader);
        while self.next_mode::<false>(reader) {}
    }

    /// Point the store at the document being parsed, which saved strings
    /// borrow from. Later calls must read the same document: the matcher and
    /// open elements carry its state, whether or not any row was saved yet.
    /// Out of line: it runs once per call, not once per tag.
    #[cfg_attr(not(scah_no_layout_hints), inline(never))]
    fn bind_source(&mut self, reader: &Reader<'html>) {
        let source = reader.source();
        if self.source_bound {
            assert!(
                self.out.store.bound_to(source),
                "a parser reads one document"
            );
        } else {
            let html = reader.source_str();
            self.out.store.set_html(html);
            // The source is borrowed for `'html`, so it cannot change while
            // the parser holds it: the indexer prepares it once.
            self.indexer.prepare(source);
            self.source_bound = true;
        }
    }

    /// Whether no later tag can change the results: every query is done and
    /// no saved element is waiting for its close tag.
    #[inline]
    fn finished(&self) -> bool {
        self.can_finish && self.capture.idle() && self.matcher.finished()
    }

    /// Process one tag: the open or close tag of an element, or the end of a
    /// raw-text element. Returns whether to continue.
    #[inline(always)]
    fn next_mode<const CAPTURE: bool>(&mut self, reader: &mut Reader<'html>) -> bool {
        if let Some(close_tag) = self.raw_text_close {
            return self.next_after_raw_text::<CAPTURE>(reader, close_tag);
        }
        let Some(tag) = self.next_tag::<CAPTURE>(reader) else {
            self.close_all_at_eof::<CAPTURE>(reader);
            return false;
        };
        let finished = match tag {
            TagEvent::Open(open) => match self.open_tag::<CAPTURE>(open, reader) {
                Ok(finished) => finished,
                Err(err) => {
                    self.record_parse_error(err);
                    return false;
                }
            },
            TagEvent::Complete(close) => {
                debug_assert_eq!(close.kind, TagKind::Close);
                reader.advance_to(close.end);
                // SAFETY: the parser's own indexer found `close` in the reader's
                // source, which is borrowed for `'html` and so unchanged.
                let name = unsafe { close.name_unchecked(reader.source_str()) };
                self.end_tag::<CAPTURE>(name, close.start, reader)
            }
        };
        self.continue_after_tag::<CAPTURE>(finished, reader)
    }

    /// The next open or close tag. Comments and declarations are skipped,
    /// but they still interrupt captured text.
    #[inline(always)]
    fn next_tag<const CAPTURE: bool>(&mut self, reader: &mut Reader<'html>) -> Option<TagEvent> {
        let source = reader.source();
        loop {
            match self.indexer.next(source, reader.get_position())? {
                TagEvent::Complete(span) if span.kind == TagKind::Ignored => {
                    self.position.reader_position = span.start;
                    self.capture.text_until::<CAPTURE>(
                        reader,
                        span.start,
                        self.position.element_depth,
                        &mut self.out,
                    );
                    self.capture.cancel_initial_newline::<CAPTURE>();
                    reader.advance_to(span.end);
                    self.capture.text_from::<CAPTURE>(reader.get_position());
                }
                tag => return Some(tag),
            }
        }
    }

    /// Open an element: close the elements its tag implicitly closes, match
    /// it, and start capturing the content of the matches it saves. Returns
    /// whether parsing is finished.
    #[inline(always)]
    fn open_tag<const CAPTURE: bool>(
        &mut self,
        open: OpenTagStart,
        reader: &mut Reader<'html>,
    ) -> Result<bool, ParseError> {
        // SAFETY: the parser's own indexer found `open` in the reader's
        // source, which is borrowed for `'html` and so unchanged.
        let name = unsafe { open.name_unchecked(reader.source_str()) };
        let id = TagId::of(name);
        let classified = ClassifiedTag::of(id);
        let tag = classified.parser;
        self.position.reader_position = open.start;
        self.element.set_name(name);
        self.capture.text_until::<CAPTURE>(
            reader,
            open.start,
            self.position.element_depth,
            &mut self.out,
        );
        let mut finished = self.close_implied_by::<CAPTURE>(tag, reader);

        let candidates = self.matcher.prepare(id, name);
        self.read_attributes::<CAPTURE>(candidates, &open, reader);
        if let Some(close_tag) = tag.raw_text_close_tag() {
            self.raw_text_close = Some(close_tag);
        }
        self.position.reader_position = reader.get_position();

        let void = tag.is_void();
        let text_tag = self.capture.text_tag::<CAPTURE>(classified.text);
        let open_text = self.capture.open_text::<CAPTURE>(&self.element, text_tag);
        self.push_element(id, tag, void, open_text.stack_flags())?;
        let structural = self
            .temp_state
            .structural
            .as_deref_mut()
            .map(|state| state.open(self.element.name, !void, &self.element));
        crate::scah_trace!(
            self.out.trace,
            TraceEvent::OpenTag {
                tag: self.element.name,
                depth: self.position.element_depth,
                reader_position: self.position.reader_position,
                self_closing: void,
            }
        );

        self.matcher.open(
            &self.element,
            structural.as_ref(),
            &mut self.out,
            &mut self.temp_state.save_hits,
        );
        self.capture.open::<CAPTURE>(
            &self.temp_state.save_hits,
            &open_text,
            void,
            self.position.reader_position,
            self.position.element_depth,
            &mut self.out,
        );
        if void {
            self.matcher.close();
        }
        finished = finished || self.finished();

        self.element.clear();
        self.capture.text_from::<CAPTURE>(reader.get_position());
        Ok(finished)
    }

    /// Parse the attributes that the prepared candidates, structural
    /// filters or text capture read, and move the reader past the open tag.
    #[inline(always)]
    fn read_attributes<const CAPTURE: bool>(
        &mut self,
        candidates: bool,
        open: &OpenTagStart,
        reader: &mut Reader<'html>,
    ) {
        let source = reader.source();
        let interest = &mut self.temp_state.attribute_interest;
        interest.clear();
        if candidates {
            interest.add(self.matcher.attribute_mask());
        }
        if let Some(structural) = self.temp_state.structural.as_ref() {
            interest.add(structural.attribute_interest);
        }
        if self.capture.needs_hidden::<CAPTURE>() {
            interest.require_hidden();
        }
        // A tag ending immediately after its name cannot carry selector
        // attributes, saved attributes, or hidden-text suppression, so it
        // needs neither tokenizing nor a search for its end.
        let end = if source.get(open.attributes_start) == Some(&b'>') {
            open.attributes_start + 1
        } else if !interest.is_empty() {
            #[cfg(test)]
            {
                self.attribute_parse_count += 1;
            }
            // SAFETY: the indexer starts attributes after the tag name, at
            // an ASCII byte or the end of the source.
            let mut attributes = Reader::new(unsafe {
                std::str::from_utf8_unchecked(&source[open.attributes_start..])
            });
            // The store copies the attributes of rows that save them, so
            // every tag parses into the same scratch list.
            self.temp_state.attributes.clear();
            // SAFETY: the parser mutates the tape only here, and no copy of
            // `element.attributes` outlives the tag: the matcher and store
            // keep the keys and values, which borrow the source, never the
            // slice. The tape is dropped with the element.
            // The reader starts at a character boundary (above).
            unsafe {
                self.element.parse_attributes(
                    &mut attributes,
                    &mut self.temp_state.attributes,
                    &self.temp_state.attribute_interest,
                );
            }
            #[cfg(test)]
            {
                self.selected_attribute_count += self.element.attributes.len();
            }
            open.attributes_start + attributes.get_position()
        } else {
            self.indexer.finish_open(source, open)
        };
        reader.advance_to(end);
    }

    /// Push the element being opened. A void element is never pushed: it is
    /// closed as soon as it is matched.
    #[inline(always)]
    fn push_element(
        &mut self,
        id: TagId,
        tag: TagFlags,
        void: bool,
        text_flags: TextElementFlags,
    ) -> Result<(), ParseError> {
        if void {
            let depth = self.open_elements.depth().saturating_add(1);
            if depth > MAX_ELEMENT_DEPTH {
                return Err(ParseError::MaximumDepthExceeded);
            }
            self.position.element_depth = depth;
        } else {
            self.open_elements
                .push_classified(self.element.name, id, tag, text_flags.bits())?;
            self.position.element_depth = self.open_elements.depth();
        }
        Ok(())
    }

    /// Handle an end tag that starts at `start`; the reader is past it.
    /// Returns whether parsing is finished.
    #[inline(always)]
    fn end_tag<const CAPTURE: bool>(
        &mut self,
        name: &'html str,
        start: usize,
        reader: &Reader<'html>,
    ) -> bool {
        self.position.reader_position = start;
        self.capture.text_until::<CAPTURE>(
            reader,
            start,
            self.position.element_depth,
            &mut self.out,
        );
        self.capture.cancel_initial_newline::<CAPTURE>();
        let finished = self.close_named::<CAPTURE>(name, reader);
        self.capture.text_from::<CAPTURE>(reader.get_position());
        finished
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
            self.close_all_at_eof::<CAPTURE>(reader);
            return false;
        };
        self.raw_text_close = None;

        // The end tag ends at the next `>`, whatever follows its name: a
        // tolerated form such as `</style ignored>` still closes the element.
        reader.advance_to(close_position);
        reader.next_until(b'>');
        reader.skip();

        let finished = self.end_tag::<CAPTURE>(&close_tag[2..], close_position, reader);
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
                self.out.trace,
                TraceEvent::EarlyExit {
                    reader_position: reader.get_position(),
                }
            );
            return false;
        }
        if reader.eof() {
            self.close_all_at_eof::<CAPTURE>(reader);
            return false;
        }
        true
    }

    pub fn matches(self) -> Store<'html, 'query> {
        self.out.store.finish()
    }

    pub fn trace_parse_started(
        &mut self,
        #[cfg_attr(not(any(debug_assertions, test)), allow(unused_variables))] html_len: usize,
        #[cfg_attr(not(any(debug_assertions, test)), allow(unused_variables))] query_count: usize,
    ) {
        crate::scah_trace!(
            self.out.trace,
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

    pub fn finish(self) -> Store<'html, 'query> {
        // Debug and test builds trace the end of the parse.
        #[cfg(any(debug_assertions, test))]
        let (store, _trace) = self.finish_traced();
        #[cfg(not(any(debug_assertions, test)))]
        let store = self.out.store.finish();
        store
    }

    /// [`XHtmlParser::finish`], also returning the trace of the parse.
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn finish_traced(self) -> (Store<'html, 'query>, TraceStore<'html, 'query>) {
        let Output { store, mut trace } = self.out;
        let tapes = store.text();
        let (raw_text_len, text_len) = (tapes.raw_text.len(), tapes.text.len());
        let store = store.finish();
        crate::scah_trace!(
            trace,
            TraceEvent::ParseFinished {
                element_count: store.len(),
                result_count: store.result_count(),
                attribute_count: store.attribute_count(),
                raw_text_len,
                text_len,
            }
        );
        (store, trace)
    }

    /// Close an element popped from the open-element stack. Returns whether
    /// parsing is finished.
    #[inline(always)]
    fn close_element<const CAPTURE: bool>(
        &mut self,
        element: OpenElement<'html>,
        depth: DepthSize,
        reader: &Reader<'html>,
    ) -> bool {
        self.capture.close::<CAPTURE>(
            TextElementFlags::from_bits(element.payload()),
            depth,
            self.position.reader_position,
            reader,
            &mut self.out,
        );
        if let Some(structural) = self.temp_state.structural.as_deref_mut() {
            structural.close();
        }
        self.position.element_depth = depth;
        self.matcher.close();
        self.finished()
    }

    /// Close the element an end tag names. Returns whether parsing is
    /// finished.
    #[inline(always)]
    fn close_named<const CAPTURE: bool>(
        &mut self,
        name: &'html str,
        reader: &Reader<'html>,
    ) -> bool {
        crate::scah_trace!(
            self.out.trace,
            TraceEvent::CloseTag {
                tag: name,
                depth: self.position.element_depth,
                reader_position: self.position.reader_position,
            }
        );

        // Well-formed markup closes the element on top of the stack, and
        // usually repeats its exact name, so try that before resolving the
        // name's id. This is `close_by_end_tag_into`'s first case, kept off
        // the `closing` list.
        if let Some(element) = self.open_elements.pop_exact_top(name) {
            let depth = self.open_elements.depth().saturating_add(1);
            return self.close_element::<CAPTURE>(element, depth, reader);
        }
        let id = TagId::of(name);
        if let Some(element) = self.open_elements.pop_matching_top(name, id) {
            let depth = self.open_elements.depth().saturating_add(1);
            return self.close_element::<CAPTURE>(element, depth, reader);
        }
        self.close_mismatched::<CAPTURE>(name, id, reader)
    }

    /// Close the elements an end tag closes when it does not name the top
    /// element: an element further down and everything above it, or nothing.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn close_mismatched<const CAPTURE: bool>(
        &mut self,
        name: &'html str,
        id: TagId,
        reader: &Reader<'html>,
    ) -> bool {
        self.open_elements
            .close_by_end_tag_into(name, id, &mut self.temp_state.closing);
        self.close_popped::<CAPTURE>(reader, ImpliedCloseReason::MismatchedEndTag, Some(name))
    }

    /// Close the elements that opening a `tag` closes implicitly, such as an
    /// open `p` before a `div`. Returns whether parsing is finished.
    #[inline(always)]
    fn close_implied_by<const CAPTURE: bool>(
        &mut self,
        tag: TagFlags,
        reader: &Reader<'html>,
    ) -> bool {
        if !tag.can_trigger_implied_close() {
            return false;
        }
        self.open_elements
            .prepare_for_open_into(tag, &mut self.temp_state.closing);
        !self.temp_state.closing.is_empty()
            && self.close_popped::<CAPTURE>(reader, ImpliedCloseReason::OpenTagRule, None)
    }

    /// Close every element still open at the end of the input, so their
    /// saved content is finished.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn close_all_at_eof<const CAPTURE: bool>(&mut self, reader: &mut Reader<'html>) {
        reader.advance_to(reader.source().len());
        if self.eof_drained {
            return;
        }
        self.capture.text_until::<CAPTURE>(
            reader,
            reader.get_position(),
            self.position.element_depth,
            &mut self.out,
        );
        self.position.reader_position = reader.get_position();
        self.open_elements
            .close_all_at_eof_into(&mut self.temp_state.closing);
        self.close_popped::<CAPTURE>(reader, ImpliedCloseReason::EofDrain, None);
        self.eof_drained = true;
    }

    /// Close the elements in `temp_state.closing`, innermost first. Elements
    /// other than the one an end tag names (`named`) are traced as implied
    /// closes. Returns whether parsing is finished.
    #[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]
    fn close_popped<const CAPTURE: bool>(
        &mut self,
        reader: &Reader<'html>,
        reason: ImpliedCloseReason,
        named: Option<&'html str>,
    ) -> bool {
        // Only the trace reads `reason`; release builds compile it out.
        let _ = reason;
        let mut closing = std::mem::take(&mut self.temp_state.closing);
        let base_depth = self.open_elements.depth();
        let total = closing.len();
        let mut finished = false;
        for (index, element) in closing.drain(..).enumerate() {
            let depth = base_depth.saturating_add((total - index) as DepthSize);
            if named.is_none_or(|named| !element.name.eq_ignore_ascii_case(named)) {
                crate::scah_trace!(
                    self.out.trace,
                    TraceEvent::ImpliedClose {
                        tag: element.name,
                        depth,
                        reason,
                    }
                );
            }
            finished = self.close_element::<CAPTURE>(element, depth, reader) || finished;
        }
        self.temp_state.closing = closing;
        finished
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
    use crate::ElementRef;
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
        assert!(store.text_tapes().text.as_bytes().is_empty());
    }

    #[test]
    fn unmatched_text_query_leaves_both_tapes_empty() {
        let html = "<main>outside &amp; text<div>more text</div></main>";
        let queries = &[Query::all(".missing", Save::all()).unwrap().build()];
        let mut parser = XHtmlParser::new(queries);
        let mut reader = Reader::new(html);

        parser.run(&mut reader);

        assert!(parser.out.store.text().raw_text.as_bytes().is_empty());
        assert!(parser.out.store.text().text.as_bytes().is_empty());
        assert_eq!(parser.capture.active_counts(), (0, 0));
        #[cfg(feature = "bench-internals")]
        {
            assert_eq!(parser.capture.text.path_stats.flush_calls, 0);
            assert_eq!(parser.capture.text.path_stats.decoded_fragments, 0);
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
        assert_eq!(
            store.text_tapes().raw_text.as_bytes(),
            b"selected &amp; nested"
        );
        assert_eq!(store.text_tapes().text.as_bytes(), b"selected & nested");
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
    fn index_ahead_skips_false_events_inside_long_raw_text() {
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
    fn index_ahead_finds_raw_close_after_tag_like_quote_inside_script() {
        let html = format!(
            "<script>{}const s = \"<div data='unterminated\";</script><span>real</span>",
            "x".repeat(20_000)
        );
        let queries = &[Query::all("span", Save::name_only()).unwrap().build()];

        let store = parse(&html, queries).unwrap();

        assert_eq!(store.get("span").unwrap().count(), 1);
    }

    #[test]
    fn index_ahead_uses_earliest_literal_raw_close() {
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
        let mut reader = Reader::from_bytes(html.as_bytes()).unwrap();

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
    fn index_ahead_trailing_bare_delimiter_still_drains_open_elements() {
        let unit = format!("{}<span></span>", "x".repeat(900));
        let html = format!("<div>{}<", unit.repeat(200));
        let queries = &[Query::all("div", Save::all()).unwrap().build()];

        let store = parse(&html, queries).unwrap();
        let div = store.get("div").unwrap().next().unwrap();

        assert_eq!(div.inner_html(), Some(&html["<div>".len()..]));
    }

    #[test]
    fn index_ahead_ignores_a_trailing_bare_open_delimiter() {
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
        assert_eq!(store.text_tapes().text.len(), 0);
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
        let mut reader = Reader::from_bytes(html.as_bytes()).unwrap();
        while parser.next(&mut reader) {}
        let store = parser.matches();
        let p = store.get("p").unwrap().next().unwrap();
        assert_eq!(p.inner_html(), Some("é"));
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
        assert_eq!(parser.out.store.len(), 0);
        let mut reader = Reader::new(&second);
        parser.next(&mut reader);
    }
}
