//! A [`TagIndexer`] that resolves whole tags from the simdlex tape before
//! the parser asks for them (experimental).
//!
//! Stage 1 is the structural tape of [`super::tape_indexer`]: the offsets of
//! every `<`, `>`, `'` and `"`. Stage 2 walks that tape once with a small
//! automaton (text, open tag, close tag, inside `"`, inside `'`) and emits
//! each tag with its end already known, in batches. The parser then never
//! searches for a tag's end.
//!
//! The automaton only handles the common shapes: `<name ...>` with quoted
//! values, and `</name ...>` without quotes. Anything else (comments, `<!`,
//! `<?`, `< x`, a closing quote after a backslash, quotes in an end tag, the
//! end of input) is handed to the scalar [`next_event`], so every event is
//! the one the scalar backend returns. A batch assumes the parser resumes
//! where the previous tag ended; when it resumes elsewhere (after raw text,
//! or when attribute tokenizing ends a tag differently), the batch is
//! dropped and the walk restarts there.

use std::ops::Range;

use simdlex::{Indexer, StructuralIndex};

use super::indexer::{
    OpenTagStart, ScalarTagIndexer, StructuralSearch, TagEvent, TagIndexer, TagKind, TagSpan,
    close_tag_name, find_raw_text_close_packed, find_unquoted_tag_end, is_name_boundary,
    next_event,
};
use super::tape_indexer::HtmlBounds;

/// Counters for `--cfg scah_resolved_stats` builds.
#[cfg(scah_resolved_stats)]
pub mod stats {
    use std::sync::atomic::AtomicUsize;
    pub static EVENTS: AtomicUsize = AtomicUsize::new(0);
    pub static RESTARTS: AtomicUsize = AtomicUsize::new(0);
    pub static SCALAR: AtomicUsize = AtomicUsize::new(0);
}

const FIRST_WINDOW_BYTES: usize = 1024;
const MAX_WINDOW_BYTES: usize = 64 * 1024;
/// Tags resolved per batch.
const BATCH: usize = 64;
/// Structurals the hot loop reads per call.
const MAX_STEPS: usize = 8 * BATCH;

/// Markup shapes that [`markup_at`] recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    Close,
    /// `<!--` up to `-->` or `--!>`.
    Comment,
    /// `<!...>` other than a comment.
    Declaration,
}

#[derive(Debug)]
pub(crate) struct ResolvedTagIndexer {
    indexer: Indexer<HtmlBounds>,
    index: StructuralIndex,
    /// The input range the tape currently covers.
    window: Range<usize>,
    next_window: usize,
    /// Next tape entry the walk reads.
    tape_position: usize,
    /// Resolved tags, and where each ends.
    events: Vec<(TagEvent, usize)>,
    head: usize,
    /// Where the parser must ask from for the next pending tag to apply:
    /// the end of the tag returned last.
    expected: usize,
    /// The walk found no further tags.
    exhausted: bool,
    /// Automaton state between batches ([`walk`] codes) and the `<` of the
    /// tag it is inside, if any.
    walk_state: u8,
    walk_start: usize,
    /// Tags the hot loop found, before they become events.
    records: Box<[Record; BATCH + 1]>,
    source_pointer: usize,
    source_len: usize,
    #[cfg(test)]
    pub(crate) restarts: usize,
}

impl Default for ResolvedTagIndexer {
    fn default() -> Self {
        Self {
            indexer: Indexer::new(),
            index: StructuralIndex::new(),
            window: 0..0,
            next_window: FIRST_WINDOW_BYTES,
            tape_position: 0,
            events: Vec::with_capacity(BATCH + 1),
            head: 0,
            expected: usize::MAX,
            exhausted: false,
            walk_state: walk::TEXT,
            walk_start: 0,
            records: Box::new([Record::default(); BATCH + 1]),
            source_pointer: 0,
            source_len: 0,
            #[cfg(test)]
            restarts: 0,
        }
    }
}

impl ResolvedTagIndexer {
    #[inline]
    fn bind(&mut self, source: &[u8]) {
        let pointer = source.as_ptr() as usize;
        if self.source_pointer != pointer || self.source_len != source.len() {
            self.source_pointer = pointer;
            self.source_len = source.len();
            self.window = 0..0;
            self.next_window = FIRST_WINDOW_BYTES;
            self.tape_position = 0;
            self.events.clear();
            self.head = 0;
            self.expected = usize::MAX;
            self.exhausted = false;
            self.walk_state = walk::TEXT;
        }
    }

    /// Index the window containing `from`, starting at its 64-byte block.
    fn index_window_at(&mut self, source: &[u8], from: usize) {
        let start = from - from % 64;
        let end = (start + self.next_window).min(source.len());
        let Ok(()) = self
            .indexer
            .index_window(source, start..end, &mut (), &mut self.index);
        self.window = start..end;
        self.tape_position = 0;
        self.next_window = (self.next_window * 2).min(MAX_WINDOW_BYTES);
    }

    /// Point the walk at the first structural at or after `from`.
    fn seek(&mut self, source: &[u8], from: usize) {
        if from >= source.len() {
            // Nothing left to index; make sure the old window's tape is not
            // read again.
            self.window = source.len()..source.len();
            self.tape_position = self.index.tape().len();
            return;
        }
        if !self.window.contains(&from) {
            self.index_window_at(source, from);
        }
        let tape = self.index.tape();
        self.tape_position = tape.partition_point(|&offset| (offset as usize) < from);
    }

    /// The next structural offset, indexing further windows as needed.
    #[inline]
    fn next_bound(&mut self, source: &[u8]) -> Option<usize> {
        loop {
            if let Some(&offset) = self.index.tape().get(self.tape_position) {
                self.tape_position += 1;
                return Some(offset as usize);
            }
            if self.window.end >= source.len() {
                return None;
            }
            let end = self.window.end;
            self.index_window_at(source, end);
        }
    }

    /// Restart the walk at `from` with no tags pending.
    fn restart(&mut self, source: &[u8], from: usize) {
        #[cfg(test)]
        {
            self.restarts += 1;
        }
        #[cfg(scah_resolved_stats)]
        stats::RESTARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.events.clear();
        self.head = 0;
        self.expected = from;
        self.exhausted = false;
        self.walk_state = walk::TEXT;
        self.seek(source, from);
    }

    /// Resolve the tag at `lt` with the scalar backend and resume after it.
    #[cold]
    #[inline(never)]
    fn resolve_scalar(&mut self, source: &[u8], lt: usize) {
        #[cfg(scah_resolved_stats)]
        stats::SCALAR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(event) = next_event(&mut ScalarTagIndexer, source, lt) else {
            self.exhausted = true;
            return;
        };
        let (event, end) = match event {
            TagEvent::Open(mut open) => {
                let end = find_unquoted_tag_end(source, open.attributes_start);
                open.end_hint = Some(end);
                (TagEvent::Open(open), end)
            }
            TagEvent::Complete(span) => {
                let end = span.end;
                (TagEvent::Complete(span), end)
            }
        };
        self.events.push((event, end));
        self.walk_state = walk::TEXT;
        self.seek(source, end);
    }

    /// Resolve up to [`BATCH`] tags from the walk's position. A batch ends
    /// early after a raw-text element's open tag: the parser skips its
    /// contents itself, and the tags the walk found in them do not exist.
    fn refill(&mut self, source: &[u8]) {
        self.events.clear();
        self.head = 0;
        while self.events.len() < BATCH && !self.exhausted {
            let (count, slow) = self.walk_fast(source);
            if self.push_records(source, count) {
                return;
            }
            match slow {
                Slow::None => {}
                Slow::Window => {
                    if self.window.end >= source.len() {
                        if self.walk_state == walk::TEXT {
                            self.exhausted = true;
                        } else {
                            self.resolve_scalar(source, self.walk_start);
                        }
                    } else {
                        let end = self.window.end;
                        self.index_window_at(source, end);
                    }
                }
                Slow::At(position) => self.resolve_slow(source, position),
            }
        }
    }

    /// The hot loop: run the automaton over the current window's tape until
    /// a batch of tags is found, the window ends, or a shape needs the slow
    /// path. No branch in it depends on the bytes except that exit.
    #[inline(never)]
    fn walk_fast(&mut self, source: &[u8]) -> (usize, Slow) {
        let tape = self.index.tape();
        let last = source.len().saturating_sub(1);
        let mut state = self.walk_state;
        let mut start = self.walk_start;
        let mut count = 0;
        let mut position = self.tape_position;
        let records = &mut *self.records;
        // Bound the walk between returns, so that the contents of a raw-text
        // element (whose tags do not exist) cost at most this much.
        let steps_end = tape.len().min(position + MAX_STEPS);
        let slow = loop {
            if position == steps_end {
                break if steps_end == tape.len() {
                    Slow::Window
                } else {
                    Slow::None
                };
            }
            // SAFETY: `position < tape.len()`.
            let at = unsafe { *tape.get_unchecked(position) } as usize;
            // SAFETY: tape offsets are inside `source`.
            let byte = unsafe { *source.get_unchecked(at) };
            // Branches on the state and byte: tags repeat the same few
            // shapes, which the branch predictor learns. A branch-free table
            // walk measured slower.
            let (next, emit) = match (state, byte) {
                (walk::TEXT, b'<') => match walk::lt_class(source, at, last) {
                    walk::LT_OPEN => (walk::OPEN, None),
                    walk::LT_CLOSE => (walk::CLOSE_TAG, None),
                    _ => break Slow::At(at),
                },
                (walk::TEXT, _) => (walk::TEXT, None),
                (walk::OPEN, b'>') => (walk::TEXT, Some(false)),
                (walk::OPEN, b'"') => (walk::DOUBLE, None),
                (walk::OPEN, b'\'') => (walk::SINGLE, None),
                (walk::OPEN, _) => (walk::OPEN, None),
                (walk::CLOSE_TAG, b'>') => (walk::TEXT, Some(true)),
                (walk::CLOSE_TAG, b'<') => (walk::CLOSE_TAG, None),
                (walk::CLOSE_TAG, _) => break Slow::At(at),
                (walk::DOUBLE, b'"') | (walk::SINGLE, b'\'') => {
                    // SAFETY: a closing quote follows its opening one.
                    if unsafe { *source.get_unchecked(at - 1) } == b'\\' {
                        break Slow::At(at);
                    }
                    (walk::OPEN, None)
                }
                (other, _) => (other, None),
            };
            position += 1;
            if state == walk::TEXT {
                start = at;
            }
            state = next;
            if let Some(close) = emit {
                records[count] = Record {
                    start: start as u32,
                    end: at as u32 + 1,
                    close,
                };
                count += 1;
                if count == BATCH {
                    break Slow::None;
                }
            }
        };
        self.tape_position = position;
        self.walk_state = state;
        self.walk_start = start;
        (count, slow)
    }

    /// Turn the first `count` records into events. Returns true if a
    /// raw-text element ended the batch.
    fn push_records(&mut self, source: &[u8], count: usize) -> bool {
        for index in 0..count {
            let Record { start, end, close } = self.records[index];
            let (start, end) = (start as usize, end as usize);
            if close {
                let (name_start, name_end) = close_tag_name(source, start + 2, end - 1);
                self.push_complete(start, end, TagKind::Close, name_start..name_end);
                continue;
            }
            let name_start = start + 1;
            let mut name_end = name_start + 1;
            while name_end < end - 1 && !is_name_boundary(source[name_end]) {
                name_end += 1;
            }
            self.events.push((
                TagEvent::Open(OpenTagStart {
                    start,
                    name: name_start..name_end,
                    attributes_start: name_end,
                    end_hint: Some(end),
                }),
                end,
            ));
            if is_raw_text(&source[name_start..name_end]) {
                return true;
            }
        }
        false
    }

    /// The structural at `position` stopped the hot loop.
    #[cold]
    #[inline(never)]
    fn resolve_slow(&mut self, source: &[u8], position: usize) {
        if self.walk_state != walk::TEXT {
            // An escaped closing quote, or a quote in an end tag.
            self.resolve_scalar(source, self.walk_start);
            return;
        }
        // Skip the `<` itself.
        self.tape_position += 1;
        let ends_comment = match markup_at(source, position) {
            Some(State::Comment) => true,
            Some(State::Declaration) => false,
            _ => return self.resolve_scalar(source, position),
        };
        while let Some(gt) = self.next_bound(source) {
            if source[gt] != b'>' {
                continue;
            }
            let prefix = &source[..gt];
            if !ends_comment || prefix.ends_with(b"--") || prefix.ends_with(b"--!") {
                self.push_complete(
                    position,
                    gt + 1,
                    TagKind::Ignored,
                    position + 1..position + 1,
                );
                return;
            }
        }
        self.resolve_scalar(source, position);
    }

    #[inline]
    fn push_complete(&mut self, start: usize, end: usize, kind: TagKind, name: Range<usize>) {
        self.events.push((
            TagEvent::Complete(TagSpan {
                start,
                end,
                kind,
                name,
            }),
            end,
        ));
    }
}

/// Why the hot loop stopped.
enum Slow {
    /// A full batch.
    None,
    /// The window's tape ran out.
    Window,
    /// The structural at this offset needs the slow path.
    At(usize),
}

/// A tag the hot loop resolved.
#[derive(Debug, Clone, Copy, Default)]
struct Record {
    start: u32,
    end: u32,
    close: bool,
}

/// The hot loop's states and the classes of a `<`.
mod walk {
    pub(super) const TEXT: u8 = 0;
    pub(super) const OPEN: u8 = 1;
    pub(super) const CLOSE_TAG: u8 = 2;
    pub(super) const DOUBLE: u8 = 3;
    pub(super) const SINGLE: u8 = 4;

    pub(super) const LT_OPEN: u8 = 0;
    pub(super) const LT_CLOSE: u8 = 1;
    const LT_SLOW: u8 = 2;

    /// Class of the `<` whose next byte is the index, as `next_event`
    /// reads it: `<!`, `<<`, whitespace and other name boundaries take the
    /// slow path.
    const LT_NEXT: [u8; 256] = {
        let mut table = [LT_OPEN; 256];
        let slow = b" \t\n\x0c\r'\"=><!";
        let mut index = 0;
        while index < slow.len() {
            table[slow[index] as usize] = LT_SLOW;
            index += 1;
        }
        table[b'/' as usize] = LT_CLOSE;
        table
    };

    /// The class of the `<` at `at`; `last` is the input's last index. A
    /// trailing `<` or `</` reads itself as its neighbour and goes slow.
    #[inline(always)]
    pub(super) fn lt_class(source: &[u8], at: usize, last: usize) -> u8 {
        // SAFETY: both indices are clamped to `0..=last`, inside `source`.
        let (next, after) = unsafe {
            (
                *source.get_unchecked((at + 1).min(last)),
                *source.get_unchecked((at + 2).min(last)),
            )
        };
        match LT_NEXT[next as usize] {
            LT_CLOSE if !after.is_ascii_alphabetic() => LT_SLOW,
            class => class,
        }
    }
}

/// The walk state for markup starting at the `<` at `lt`, as the scalar
/// `next_event` reads it, or `None` for shapes left to `next_event` itself.
#[inline]
fn markup_at(source: &[u8], lt: usize) -> Option<State> {
    match *source.get(lt + 1)? {
        b'/' => source
            .get(lt + 2)
            .is_some_and(u8::is_ascii_alphabetic)
            .then_some(State::Close),
        b'!' => {
            if source.get(lt + 2..lt + 4) != Some(b"--") {
                return Some(State::Declaration);
            }
            // `<!-->` and `<!--->` end early.
            let after = source.get(lt + 4..lt + 6);
            (source.get(lt + 4) != Some(&b'>') && after != Some(b"->")).then_some(State::Comment)
        }
        // `<` then whitespace or `<` skips ahead, and an empty name skips
        // the whole tag.
        b'<' => None,
        byte if is_name_boundary(byte) => None,
        // Any other byte starts a name, as in `<3` or `<?xml`.
        _ => Some(State::Open),
    }
}

/// Elements whose contents the parser skips to their close tag.
#[inline]
fn is_raw_text(name: &[u8]) -> bool {
    matches!(name.len(), 5 | 6 | 8)
        && ["script", "style", "textarea", "title"]
            .iter()
            .any(|raw| raw.as_bytes().eq_ignore_ascii_case(name))
}

impl StructuralSearch for ResolvedTagIndexer {
    fn find_byte(&mut self, source: &[u8], from: usize, needle: u8) -> Option<usize> {
        // Only raw-text searches get here; they move the walk, so the next
        // `next` restarts it.
        self.expected = usize::MAX;
        self.seek(source, from);
        while let Some(position) = self.next_bound(source) {
            if source[position] == needle {
                return Some(position);
            }
        }
        None
    }

    fn find_tag_end(&mut self, source: &[u8], from: usize) -> usize {
        find_unquoted_tag_end(source, from)
    }
}

impl TagIndexer for ResolvedTagIndexer {
    #[inline]
    fn prepare(&mut self, source: &[u8]) {
        self.bind(source);
    }

    #[cfg_attr(not(scah_no_layout_hints), inline(never))]
    fn next(&mut self, source: &[u8], from: usize) -> Option<TagEvent> {
        self.bind(source);
        if from != self.expected {
            self.restart(source, from);
        }
        if self.head == self.events.len() {
            if self.exhausted {
                return None;
            }
            self.refill(source);
            if self.head == self.events.len() {
                return None;
            }
        }
        #[cfg(scah_resolved_stats)]
        stats::EVENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (event, end) = self.events[self.head].clone();
        self.head += 1;
        self.expected = end;
        Some(event)
    }

    #[cfg_attr(not(scah_no_layout_hints), inline(never))]
    fn finish_open(&mut self, source: &[u8], open: &OpenTagStart) -> usize {
        open.end_hint
            .unwrap_or_else(|| find_unquoted_tag_end(source, open.attributes_start))
    }

    #[cfg_attr(not(scah_no_layout_hints), inline(never))]
    fn find_raw_text_close(
        &mut self,
        source: &[u8],
        from: usize,
        close_tag: &str,
    ) -> Option<usize> {
        find_raw_text_close_packed(self, source, from, close_tag.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::tape_indexer::tests::{EDGE_CASES, Rng, random_document};

    /// The event without its end hint, which only this backend provides;
    /// the tests compare ends through `finish_open`.
    fn unhinted(event: Option<TagEvent>) -> Option<TagEvent> {
        event.map(|event| match event {
            TagEvent::Open(open) => TagEvent::Open(OpenTagStart {
                end_hint: None,
                ..open
            }),
            complete => complete,
        })
    }

    /// Shapes the walk resolves itself rather than handing to `next_event`.
    const WALK_CASES: &[&str] = &[
        "a <3 b <?xml version='1.0'?> c",
        "<!DOCTYPE html><!doctype x 'y>z'><p>",
        "<!--a-b--><!---x>--><!--a--!><!----><i>",
        "<!-- '\"<p>\"' --><p a='<!--'>-->x</p>",
        "<TITLE>a<b>c</TITLE ><textarea x='>'>d</textarea>",
        "<a b=\"c\\\">\" d='e\\'>f</a>",
        "<a\tb='c'\n>x</a\n><p/><p//>",
    ];

    #[test]
    fn matches_scalar_from_every_offset() {
        for source in EDGE_CASES.iter().chain(WALK_CASES) {
            let bytes = source.as_bytes();
            for from in 0..=bytes.len() {
                let mut resolved = ResolvedTagIndexer::default();
                let expected = ScalarTagIndexer.next(bytes, from);
                let actual = resolved.next(bytes, from);
                assert_eq!(unhinted(actual.clone()), expected, "{source:?} from {from}");
                if let (Some(TagEvent::Open(expected)), Some(TagEvent::Open(actual))) =
                    (&expected, &actual)
                {
                    assert_eq!(
                        resolved.finish_open(bytes, actual),
                        ScalarTagIndexer.finish_open(bytes, expected),
                        "{source:?} from {from}"
                    );
                }
            }
        }
    }

    /// Step through documents as the parser does: from each tag's end, and
    /// sometimes from elsewhere (raw text, attribute tokenizing).
    #[test]
    fn one_indexer_matches_scalar_while_stepping_through_large_documents() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for round in 0..8 {
            let source = random_document(&mut rng, 2_000 + round * 4_000);
            let bytes = source.as_bytes();
            let mut resolved = ResolvedTagIndexer::default();
            let mut from = 0;
            let mut events = 0;
            loop {
                resolved.prepare(bytes);
                let expected = ScalarTagIndexer.next(bytes, from);
                let actual = resolved.next(bytes, from);
                assert_eq!(
                    unhinted(actual.clone()),
                    expected,
                    "round {round}, from {from}"
                );
                from = match expected {
                    None => break,
                    Some(TagEvent::Complete(span)) => span.end.max(from + 1),
                    Some(TagEvent::Open(open)) => {
                        let end = ScalarTagIndexer.finish_open(bytes, &open);
                        let Some(TagEvent::Open(actual)) = &actual else {
                            unreachable!()
                        };
                        assert_eq!(resolved.finish_open(bytes, actual), end);
                        end.max(from + 1)
                    }
                };
                match rng.below(60) {
                    0 => from = (from + rng.below(20_000)).min(bytes.len()),
                    1 => from = from.saturating_sub(rng.below(8)),
                    _ => {}
                }
                events += 1;
            }
            assert!(bytes.len() < 10_000 || events > 100, "round {round}");
            assert!(
                resolved.restarts < events / 4 + 10,
                "round {round}: too many restarts"
            );
        }
    }

    #[test]
    fn raw_text_search_then_next_restarts_the_walk() {
        let source = "<script>a<b x='>'></script ><p class=\"x\">t</p>";
        let bytes = source.as_bytes();
        let mut resolved = ResolvedTagIndexer::default();
        assert!(matches!(resolved.next(bytes, 0), Some(TagEvent::Open(_))));
        let close = resolved.find_raw_text_close(bytes, 8, "</script");
        assert_eq!(
            close,
            ScalarTagIndexer.find_raw_text_close(bytes, 8, "</script")
        );
        let after = bytes.len() - "<p class=\"x\">t</p>".len();
        assert_eq!(
            unhinted(resolved.next(bytes, after)),
            ScalarTagIndexer.next(bytes, after)
        );
    }
}
