//! A [`TagIndexer`] backed by a simdjson-style structural tape (simdlex).
//!
//! Stage 1 records the offset of every `<`, `>`, `'`, and `"` with SIMD. The
//! tag semantics are unchanged: [`next_event`] and the quote rules are the
//! ones every other backend uses; this backend only answers their searches
//! ("the next `>` at or after here") from the tape instead of scanning bytes.
//!
//! The tape is built lazily in windows, starting small and doubling, so a
//! parse that stops after its first match only indexes what it read, and a
//! search that jumps ahead (a raw-text close, an attribute run) does not
//! index the bytes it skipped.

use std::cell::Cell;
use std::convert::Infallible;
use std::ops::Range;

use simdlex::tablegen::{ByteSet, Tables, generate, unwrap};
use simdlex::{IndexSpec, Indexer, NibbleTables, StructuralIndex, TapeCursor};

use super::indexer::{
    OpenTagStart, StructuralSearch, TagEvent, TagIndexer, find_raw_text_close_packed, next_event,
};

/// The bytes the tag searches look for. They are one class: every search
/// checks the byte at each candidate anyway.
const BOUNDS: [ByteSet; 1] = [ByteSet::from_bytes(b"<>'\"")];
const TABLES: Tables<1> = unwrap(generate(&BOUNDS));

struct HtmlBounds;

impl IndexSpec for HtmlBounds {
    type State = ();
    type Error = Infallible;

    const TABLES: NibbleTables = NibbleTables {
        low: TABLES.low,
        high: TABLES.high,
    };
    const CLASS_BITS: [u8; 8] = TABLES.padded_class_bits();
    const CLASS_COUNT: usize = BOUNDS.len();
}

const FIRST_WINDOW_BYTES: usize = 1024;
const MAX_WINDOW_BYTES: usize = 64 * 1024;

thread_local! {
    /// A dropped indexer's tape, lent to the next parse on this thread so
    /// that each parse does not allocate (and fault in) a fresh tape.
    static SPARE_INDEX: Cell<Option<StructuralIndex>> = const { Cell::new(None) };
}

#[derive(Debug)]
pub(crate) struct TapeTagIndexer {
    indexer: Indexer<HtmlBounds>,
    index: StructuralIndex,
    /// The input range the tape currently covers.
    window: Range<usize>,
    /// Size of the next window; doubles up to [`MAX_WINDOW_BYTES`].
    next_window: usize,
    /// Tape position of the last search, so the next one resumes there.
    cursor: usize,
    source_pointer: usize,
    source_len: usize,
}

impl Default for TapeTagIndexer {
    fn default() -> Self {
        Self {
            indexer: Indexer::new(),
            index: SPARE_INDEX
                .try_with(Cell::take)
                .ok()
                .flatten()
                .unwrap_or_default(),
            window: 0..0,
            next_window: FIRST_WINDOW_BYTES,
            cursor: 0,
            source_pointer: 0,
            source_len: 0,
        }
    }
}

impl Drop for TapeTagIndexer {
    fn drop(&mut self) {
        let index = std::mem::take(&mut self.index);
        let _ = SPARE_INDEX.try_with(|spare| spare.set(Some(index)));
    }
}

impl TapeTagIndexer {
    /// Forget the tape when the source changes. A parser is bound to one
    /// document, so this only runs once per parse.
    #[inline]
    fn bind(&mut self, source: &[u8]) {
        let pointer = source.as_ptr() as usize;
        if self.source_pointer != pointer || self.source_len != source.len() {
            self.source_pointer = pointer;
            self.source_len = source.len();
            self.window = 0..0;
            self.next_window = FIRST_WINDOW_BYTES;
            self.cursor = 0;
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
        self.cursor = 0;
        self.next_window = (self.next_window * 2).min(MAX_WINDOW_BYTES);
    }

    /// The first indexed byte at or after `from` that satisfies `wanted`.
    #[inline]
    fn find_matching(
        &mut self,
        source: &[u8],
        from: usize,
        wanted: impl Fn(u8) -> bool,
    ) -> Option<usize> {
        self.bind(source);
        self.find_on_tape(source, from, wanted)
    }

    #[inline]
    fn find_on_tape(
        &mut self,
        source: &[u8],
        mut from: usize,
        wanted: impl Fn(u8) -> bool,
    ) -> Option<usize> {
        loop {
            if from >= source.len() {
                return None;
            }
            if !self.window.contains(&from) {
                self.index_window_at(source, from);
            }
            let mut cursor = TapeCursor::at(self.index.tape(), self.cursor);
            let found = cursor.find(source, from, &wanted);
            self.cursor = cursor.index();
            if found.is_some() {
                return found;
            }
            from = self.window.end;
        }
    }
}

impl StructuralSearch for TapeTagIndexer {
    #[inline]
    fn find_byte(&mut self, source: &[u8], from: usize, needle: u8) -> Option<usize> {
        debug_assert!(matches!(needle, b'<' | b'>' | b'\'' | b'"'));
        self.find_matching(source, from, |byte| byte == needle)
    }

    /// The end of an open tag, skipping quoted values whose closing quote is
    /// not escaped by an odd backslash run (as [`super::indexer`]'s scalar
    /// `find_unquoted_tag_end` does).
    fn find_tag_end(&mut self, source: &[u8], from: usize) -> usize {
        let mut position = from;
        let mut quote = None;

        while let Some(candidate) =
            self.find_matching(source, position, |byte| matches!(byte, b'\'' | b'"' | b'>'))
        {
            let byte = source[candidate];
            match quote {
                Some(delimiter) if byte == delimiter => {
                    let mut backslashes = 0;
                    let mut scan = candidate;
                    while scan > 0 && source[scan - 1] == b'\\' {
                        backslashes += 1;
                        scan -= 1;
                    }
                    if backslashes % 2 == 0 {
                        quote = None;
                    }
                }
                Some(_) => {}
                None if matches!(byte, b'\'' | b'"') => quote = Some(byte),
                None => return candidate + 1,
            }
            position = candidate + 1;
        }

        source.len()
    }
}

impl TagIndexer for TapeTagIndexer {
    #[inline]
    fn prepare(&mut self, source: &[u8]) {
        self.bind(source);
    }

    // Kept out of line so that adding this backend does not grow the
    // dispatcher in `AutoTagIndexer`, whose rolling path is per-tag hot.
    #[inline(never)]
    fn next(&mut self, source: &[u8], from: usize) -> Option<TagEvent> {
        next_event(self, source, from)
    }

    #[inline(never)]
    fn finish_open(&mut self, source: &[u8], open: &OpenTagStart) -> usize {
        open.end_hint
            .unwrap_or_else(|| self.find_tag_end(source, open.attributes_start))
    }

    #[inline(never)]
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
    use crate::html::indexer::ScalarTagIndexer;

    const EDGE_CASES: &[&str] = &[
        "",
        "plain text without markup",
        "0123456789abcde<a x='y'>body</a>",
        r#"prefix <a title=\"a > b\" data-x='c \\' > d'>body</a> suffix"#,
        "before<!-- a > b --!><p>after",
        "<!--><!---><!-- x --><p>",
        "<!doctype html><main><img src=x/><br></main>",
        "<<<  div class=x><span>nested</span></div>",
        "unterminated <tag attr='value",
        "text ending in a bare <",
        "text ending in repeated <<<",
        "close </  div  > after",
        r#"<div>a</div title=">" x='>'><p>b</p>"#,
        r#"<div>a</bogus data=can't><span>b</span>"#,
        r#"<p></p a"b x = 'y>z' c=d=e"f><i>"#,
        r#"<p></bogus ="x><span>ok</span>"#,
        "<x/y id=a>t</x/y><p>c</p>",
        "<><p>",
        "< ><p>",
        "<<>><p>",
        "multibyte é☃ <article data-name='é'>text</article>",
    ];

    fn assert_matches_scalar_at(bytes: &[u8], from: usize, tape: &mut TapeTagIndexer) {
        let mut scalar = ScalarTagIndexer;
        let expected = scalar.next(bytes, from);
        let actual = tape.next(bytes, from);
        assert_eq!(actual, expected, "from={from}");
        if let (Some(TagEvent::Open(expected_open)), Some(TagEvent::Open(actual_open))) =
            (&expected, &actual)
        {
            assert_eq!(
                tape.finish_open(bytes, actual_open),
                scalar.finish_open(bytes, expected_open),
                "open end differs at from={from}"
            );
        }
    }

    #[test]
    fn matches_scalar_from_every_offset() {
        for source in EDGE_CASES {
            let bytes = source.as_bytes();
            for from in 0..=bytes.len() {
                assert_matches_scalar_at(bytes, from, &mut TapeTagIndexer::default());
            }
        }
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Random markup built from tag fragments, quotes, escapes, comments,
    /// and long text runs, so tags straddle window and block boundaries.
    fn random_document(rng: &mut Rng, pieces: usize) -> String {
        const PIECES: &[&str] = &[
            "<div class=\"a b\">",
            "</div>",
            "<p>",
            "</p >",
            "<a href='x>y' title=\"q\\\"z\">",
            "<img src=x/>",
            "<!-- c > d -->",
            "<!doctype html>",
            "< b>",
            "<<i>",
            "</ x class=\">\">",
            "it's ",
            "a > b ",
            "\"quoted\" ",
            "<script>if (a < b) { s = \"</div>\"; }</script>",
            "é☃ ",
        ];
        let mut out = String::new();
        for _ in 0..pieces {
            if rng.below(8) == 0 {
                out.push_str(&"x".repeat(rng.below(5_000)));
            }
            out.push_str(PIECES[rng.below(PIECES.len())]);
        }
        out
    }

    #[test]
    fn one_indexer_matches_scalar_while_stepping_through_large_documents() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for round in 0..6 {
            let source = random_document(&mut rng, 2_000 + round * 4_000);
            let bytes = source.as_bytes();
            let mut tape = TapeTagIndexer::default();
            let mut scalar = ScalarTagIndexer;
            let mut from = 0;
            let mut events = 0;
            loop {
                tape.prepare(bytes);
                let expected = scalar.next(bytes, from);
                let actual = tape.next(bytes, from);
                assert_eq!(actual, expected, "round {round}, from {from}");
                from = match expected {
                    None => break,
                    Some(TagEvent::Complete(span)) => span.end.max(from + 1),
                    Some(TagEvent::Open(open)) => {
                        let end = scalar.finish_open(bytes, &open);
                        let Some(TagEvent::Open(actual_open)) = &actual else {
                            unreachable!()
                        };
                        assert_eq!(tape.finish_open(bytes, actual_open), end);
                        end.max(from + 1)
                    }
                };
                // Sometimes jump ahead, as the parser does after raw text.
                if rng.below(50) == 0 {
                    from = (from + rng.below(20_000)).min(bytes.len());
                }
                events += 1;
            }
            assert!(bytes.len() < 10_000 || events > 100, "round {round}");
        }
    }

    #[test]
    fn raw_text_search_matches_scalar_across_windows() {
        let long = format!(
            "{}</scripts>{}</ScRiPt >tail",
            "x < y \"q\" ".repeat(20_000),
            "z".repeat(70_000)
        );
        for source in [
            "x </scripts> <fake> y </ScRiPt >tail",
            long.as_str(),
            &format!("{}<scripted>", "x < y ".repeat(1_024)),
        ] {
            let bytes = source.as_bytes();
            let expected = ScalarTagIndexer.find_raw_text_close(bytes, 0, "</script");
            let mut tape = TapeTagIndexer::default();
            assert_eq!(tape.find_raw_text_close(bytes, 0, "</script"), expected);
            // From the middle, after the tape has moved past earlier windows.
            let middle = bytes.len() / 2;
            assert_eq!(
                tape.find_raw_text_close(bytes, middle, "</script"),
                ScalarTagIndexer.find_raw_text_close(bytes, middle, "</script")
            );
        }
    }

    #[test]
    fn a_new_source_resets_the_tape() {
        let mut tape = TapeTagIndexer::default();
        let first = "<a>".repeat(3_000);
        let second = String::from("text <b x='>'>");
        assert!(tape.next(first.as_bytes(), 8_000).is_some());
        tape.prepare(second.as_bytes());
        assert_matches_scalar_at(second.as_bytes(), 0, &mut tape);
    }
}
