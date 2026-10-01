//! Structural scanning of selector source text.
//!
//! Every place that cuts selector source into pieces (selector lists,
//! functional pseudo-class arguments, `An+B of S` filters) goes through
//! [`scan`], so a delimiter inside a quoted string, an escape, a comment or a
//! nested `()` / `[]` block can never split a selector.

use crate::query::compiler::SelectorParseError;

/// Syntax seen during a [`scan`] that matters to callers.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ScanSummary {
    /// Position of the first backslash escape, inside or outside a string.
    pub escape: Option<usize>,
    /// Position of the first `/*` comment outside a string.
    pub comment: Option<usize>,
    /// Position of the first `&` outside a string.
    pub nesting_selector: Option<usize>,
    /// Position of the first U+0000, which CSS replaces with U+FFFD.
    pub nul: Option<usize>,
    /// Position of the first `|` outside any block: a namespace prefix or a
    /// column combinator. Inside `[...]` it is an attribute operator.
    pub namespace: Option<usize>,
    /// A quoted string runs to the end of the source.
    pub unclosed_quote: bool,
}

/// Walks selector source the way the CSS tokenizer groups it: quoted
/// strings, backslash escapes and comments are opaque, and `(...)`, `[...]`
/// and `{...}` blocks nest. `visit` receives every byte outside those constructs while no
/// block is open (including a stray closer) and stops the scan by returning
/// `true`. Returns the index where the scan stopped, if it did.
pub(crate) fn scan(
    source: &str,
    mut visit: impl FnMut(usize, u8) -> bool,
) -> (Option<usize>, ScanSummary) {
    let bytes = source.as_bytes();
    let mut summary = ScanSummary::default();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'\\' => {
                summary.escape.get_or_insert(index);
                index = skip_escape(bytes, index + 1);
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                summary.comment.get_or_insert(index);
                index = bytes[index + 2..]
                    .windows(2)
                    .position(|pair| pair == b"*/")
                    .map_or(bytes.len(), |end| index + 2 + end + 2);
                continue;
            }
            b'"' | b'\'' => {
                index = skip_string(bytes, index, &mut summary);
                continue;
            }
            _ => {}
        }
        if byte == 0 {
            summary.nul.get_or_insert(index);
        } else if byte == b'&' {
            summary.nesting_selector.get_or_insert(index);
        }
        if blocks.is_empty() {
            if byte == b'|' {
                summary.namespace.get_or_insert(index);
            }
            if visit(index, byte) {
                return (Some(index), summary);
            }
        }
        match byte {
            b'(' => blocks.push(b')'),
            b'[' => blocks.push(b']'),
            b'{' => blocks.push(b'}'),
            b')' | b']' | b'}' if blocks.last() == Some(&byte) => {
                blocks.pop();
            }
            _ => {}
        }
        index += 1;
    }
    (None, summary)
}

/// Splits a selector list on its top-level commas.
pub(crate) fn split_selector_list(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    scan(source, |index, byte| {
        if byte == b',' {
            parts.push(&source[start..index]);
            start = index + 1;
        }
        false
    });
    parts.push(&source[start..]);
    parts
}

/// Returns an unsupported error when `source` contains syntax the selector
/// tokenizer does not model. Such a selector may be valid CSS that the
/// tokenizer misreads, so it can never be classified as invalid.
pub(crate) fn unmodeled_syntax_error(source: &str) -> Option<SelectorParseError> {
    let (_, summary) = scan(source, |_, _| false);
    if let Some(position) = summary.escape {
        Some(SelectorParseError::unsupported(
            "CSS escapes are not supported",
            position,
        ))
    } else if let Some(position) = summary.comment {
        Some(SelectorParseError::unsupported(
            "CSS comments are not supported",
            position,
        ))
    } else if let Some(position) = summary.nesting_selector {
        Some(SelectorParseError::unsupported(
            "the nesting selector '&' is not supported",
            position,
        ))
    } else if let Some(position) = summary.nul {
        Some(SelectorParseError::unsupported(
            "NUL characters are not supported",
            position,
        ))
    } else {
        summary.namespace.map(|position| {
            SelectorParseError::unsupported("namespaces are not supported", position)
        })
    }
}

/// Gates a parse result on modeled syntax. Fatal errors pass through
/// unchanged. Otherwise, when `source` contains syntax the tokenizer does not
/// model, both an "invalid" verdict and a success are untrustworthy, so the
/// selector is rejected as unsupported. This keeps forgiving selector lists
/// from discarding alternatives that are actually valid.
pub(crate) fn require_modeled_syntax<T>(
    source: &str,
    result: Result<T, SelectorParseError>,
) -> Result<T, SelectorParseError> {
    match result {
        Err(error) if error.is_fatal() => Err(error),
        result => match unmodeled_syntax_error(source) {
            Some(error) => Err(error),
            None => result,
        },
    }
}

/// Skips a quoted string starting at `start`, returning the index after its
/// closing quote (or the end of the source when it is unclosed).
fn skip_string(bytes: &[u8], start: usize, summary: &mut ScanSummary) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            byte if byte == quote => return index + 1,
            b'\\' => {
                summary.escape.get_or_insert(index);
                index = skip_escape(bytes, index + 1);
            }
            byte => {
                if byte == 0 {
                    summary.nul.get_or_insert(index);
                }
                index += 1;
            }
        }
    }
    summary.unclosed_quote = true;
    bytes.len()
}

/// Skips the body of an escape whose backslash precedes `index`: up to six hex
/// digits and one optional whitespace (CRLF counts as one), or one other
/// character.
fn skip_escape(bytes: &[u8], mut index: usize) -> usize {
    let hex_start = index;
    while index < bytes.len() && index - hex_start < 6 && bytes[index].is_ascii_hexdigit() {
        index += 1;
    }
    if index == hex_start {
        // Never stop inside a multi-byte character.
        if index < bytes.len() {
            index += 1;
            while index < bytes.len() && (bytes[index] & 0xC0) == 0x80 {
                index += 1;
            }
        }
    } else if bytes.get(index) == Some(&b'\r') {
        index += 1;
        if bytes.get(index) == Some(&b'\n') {
            index += 1;
        }
    } else if bytes
        .get(index)
        .is_some_and(|&byte| super::is_css_whitespace(byte))
    {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_only_on_top_level_commas() {
        for (source, expected) in [
            (".a, .b", &[".a", " .b"][..]),
            (r#"[x="a,b"], .c"#, &[r#"[x="a,b"]"#, " .c"]),
            (r"[x=a\,b], .c", &[r"[x=a\,b]", " .c"]),
            (r".a\,b", &[r".a\,b"]),
            (":not(.a, .b), .c", &[":not(.a, .b)", " .c"]),
            ("[x=a,b], .c", &["[x=a,b]", " .c"]),
            (".a/*,*/, .b", &[".a/*,*/", " .b"]),
            (r#"[x="a\",b"], .c"#, &[r#"[x="a\",b"]"#, " .c"]),
            ("[x=(], .c", &["[x=(], .c"]),
        ] {
            assert_eq!(split_selector_list(source), expected, "{source}");
        }
    }

    #[test]
    fn escaped_closers_do_not_end_blocks() {
        let (stop, _) = scan(r"[x=a\]b]), .c", |_, byte| byte == b')');
        assert_eq!(stop, Some(8));
        let (stop, _) = scan(r".a\), .c)", |_, byte| byte == b')');
        assert_eq!(stop, Some(8));
    }

    #[test]
    fn reports_unmodeled_syntax() {
        for source in [
            r":\6e ot(.a)",
            r#"[x="a\"b"]"#,
            ".a/**/.b",
            ".a/*",
            "&.a",
            ":is(&)",
            ".a\0",
            "|a",
            "ns|a",
            "col || td",
        ] {
            let error = unmodeled_syntax_error(source).expect(source);
            assert!(error.is_fatal(), "{source}");
        }
        for source in [
            r#"[x="/*"]"#,
            r#"[x="&"]"#,
            ".a .b",
            "[x=a]",
            "[x|=a]",
            r#"[x="|"]"#,
        ] {
            assert!(unmodeled_syntax_error(source).is_none(), "{source}");
        }
    }
}
