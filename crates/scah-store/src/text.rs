use std::ops::Range;

/// Append-only shared text storage for one representation (`raw_text` or `text`).
///
/// Once an element range has been finalized, previously emitted bytes must not
/// be removed, shifted, or rewritten. Separator canonicalization must therefore
/// occur in pending parser state before bytes are appended.
///
/// The tape is always valid UTF-8, so reading a range back only checks that
/// its ends fall on character boundaries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TextTape {
    content: String,
}

impl TextTape {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    #[inline]
    #[allow(dead_code)] // required tape API; used by capacity tests and callers
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    #[inline]
    #[allow(dead_code)] // required tape API; used by capacity tests and callers
    pub fn capacity(&self) -> usize {
        self.content.capacity()
    }

    /// # Panics
    ///
    /// If `range` is out of bounds or does not fall on character boundaries.
    #[inline]
    pub fn slice(&self, range: Range<usize>) -> &str {
        &self.content[range]
    }

    /// Empty the tape, keeping its allocation.
    pub fn clear(&mut self) {
        self.content.clear();
    }

    /// Make room for `additional` more bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.content.reserve(additional);
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.content.as_bytes()
    }

    /// Append bytes cut from UTF-8 text without re-validating them.
    ///
    /// # Safety
    ///
    /// `bytes` must be valid UTF-8: cut from a `str` at character boundaries.
    #[inline]
    pub unsafe fn push_bytes(&mut self, bytes: &[u8]) {
        // SAFETY: the caller guarantees `bytes` is valid UTF-8, so the
        // content stays valid UTF-8.
        unsafe { self.content.as_mut_vec() }.extend_from_slice(bytes);
    }

    /// Append an ASCII byte, such as a separator.
    ///
    /// # Panics
    ///
    /// If `byte` is not ASCII. Callers pass constants, so the check folds away.
    #[inline]
    pub fn push_byte(&mut self, byte: u8) {
        assert!(byte.is_ascii(), "the text tape only takes ASCII bytes");
        self.content.push(byte as char);
    }

    #[inline]
    pub fn push_str(&mut self, text: &str) {
        self.content.push_str(text);
    }

    #[inline]
    pub fn last_byte(&self) -> Option<u8> {
        self.content.as_bytes().last().copied()
    }
}

/// Result-only storage for both text extraction modes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TextStore {
    pub raw_text: TextTape,
    pub text: TextTape,
}

/// Trim leading/trailing collapsible separators from a normalized range
/// without mutating the shared tape.
///
/// Only space, tab, and line break are removed. Must not be applied to
/// selected preformatted (`pre` / `textarea`) ranges.
#[inline]
pub fn trim_collapsed_range(tape: &TextTape, range: Range<usize>) -> Range<usize> {
    let bytes = tape.as_bytes();
    if range.start >= range.end || range.end > bytes.len() {
        return range.start..range.start;
    }

    let mut start = range.start;
    let mut end = range.end;

    while start < end && is_normalized_separator(bytes[start]) {
        start += 1;
    }
    while end > start && is_normalized_separator(bytes[end - 1]) {
        end -= 1;
    }

    start..end
}

/// Compatibility alias used by older call sites / tests.
#[allow(dead_code)]
pub(crate) fn trim_normalized_range(tape: &TextTape, range: Range<usize>) -> Range<usize> {
    trim_collapsed_range(tape, range)
}

#[inline]
fn is_normalized_separator(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tape_slice_and_push() {
        let mut tape = TextTape::new();
        tape.push_str("Hello");
        tape.push_byte(b' ');
        tape.push_str("world");
        assert_eq!(tape.slice(0..tape.len()), "Hello world");
        assert_eq!(tape.slice(6..11), "world");
    }

    #[test]
    fn trim_collapsed_range_strips_edges() {
        let mut tape = TextTape::new();
        tape.push_str("\nA  B\t\n");
        assert_eq!(trim_collapsed_range(&tape, 0..tape.len()), 1..5);
        assert_eq!(tape.slice(1..5), "A  B");
    }

    #[test]
    fn trim_empty_stays_empty() {
        let tape = TextTape::new();
        assert_eq!(trim_collapsed_range(&tape, 0..0), 0..0);
    }
}
