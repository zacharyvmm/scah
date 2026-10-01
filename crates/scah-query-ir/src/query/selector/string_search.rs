#[derive(Debug, PartialEq, Clone)]
pub enum AttributeSelectionKind {
    Presence,            // [attribute]
    Exact,               // [attribute=value]
    WhitespaceSeparated, // [attribute~=value]
    HyphenSeparated,     // [attribute|=value]
    Prefix,              // [attribute^=value]
    Suffix,              // [attribute$=value]
    Substring,           // [attribute*=value]
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AttributeCaseSensitivity {
    Default,
    AsciiInsensitive,
    Sensitive,
}

impl AttributeSelectionKind {
    pub fn find(&self, query: &str, source: &str) -> bool {
        match self {
            Self::Exact => query == source,
            Self::Presence => true,
            Self::WhitespaceSeparated => css_words(source).any(|word| word == query),
            Self::HyphenSeparated => {
                source == query
                    || source
                        .strip_prefix(query)
                        .is_some_and(|rest| rest.starts_with('-'))
            }
            Self::Prefix => !query.is_empty() && source.starts_with(query),
            Self::Suffix => !query.is_empty() && source.ends_with(query),
            Self::Substring => !query.is_empty() && source.contains(query),
        }
    }

    pub fn find_ascii_insensitive(&self, query: &str, source: &str) -> bool {
        match self {
            Self::Exact => ascii_eq(query, source),
            Self::Presence => true,
            Self::WhitespaceSeparated => css_words(source).any(|word| ascii_eq(query, word)),
            Self::HyphenSeparated => {
                ascii_eq(query, source)
                    || source
                        .get(..query.len())
                        .is_some_and(|prefix| ascii_eq(query, prefix))
                        && source.as_bytes().get(query.len()) == Some(&b'-')
            }
            Self::Prefix => {
                !query.is_empty()
                    && source
                        .get(..query.len())
                        .is_some_and(|prefix| ascii_eq(query, prefix))
            }
            Self::Suffix => {
                !query.is_empty()
                    && source
                        .get(source.len().saturating_sub(query.len())..)
                        .is_some_and(|suffix| ascii_eq(query, suffix))
            }
            Self::Substring => {
                !query.is_empty()
                    && source
                        .as_bytes()
                        .windows(query.len())
                        .any(|w| ascii_eq_bytes(query.as_bytes(), w))
            }
        }
    }
}

/// Splits a whitespace-separated attribute value into its non-empty words.
///
/// CSS and HTML only treat space, tab, line feed, form feed, and carriage
/// return as separators, so Unicode spaces such as U+00A0 stay inside a word.
#[inline]
pub(crate) fn css_words(source: &str) -> impl Iterator<Item = &str> {
    source.split_ascii_whitespace()
}

#[inline]
fn ascii_eq(left: &str, right: &str) -> bool {
    ascii_eq_bytes(left.as_bytes(), right.as_bytes())
}

#[inline]
fn ascii_eq_bytes(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presence() {
        let kind = AttributeSelectionKind::Presence;
        assert!(kind.find("", "Hello"));
    }

    #[test]
    fn test_exact() {
        let kind = AttributeSelectionKind::Exact;
        assert!(kind.find("Hello", "Hello"));
    }

    #[test]
    fn test_whitespace() {
        let kind = AttributeSelectionKind::WhitespaceSeparated;
        assert!(kind.find("world", "hello world in test"));
    }

    #[test]
    fn whitespace_separated_splits_on_every_css_whitespace_character() {
        let kind = AttributeSelectionKind::WhitespaceSeparated;
        for source in [
            "bar foo",
            "bar\tfoo",
            "bar\nfoo",
            "bar\u{000C}foo",
            "bar\rfoo",
            " \t\r\n\u{000C}foo\u{000C}\n\r\t ",
        ] {
            assert!(kind.find("foo", source), "{source:?}");
            assert!(kind.find_ascii_insensitive("FOO", source), "{source:?}");
        }
    }

    #[test]
    fn whitespace_separated_keeps_unicode_separators_inside_words() {
        let kind = AttributeSelectionKind::WhitespaceSeparated;
        for source in [
            "bar\u{00A0}foo",
            "bar\u{000B}foo",
            "bar\u{0085}foo",
            "bar\u{1680}foo",
            "bar\u{2003}foo",
            "bar\u{2028}foo",
            "bar\u{3000}foo",
        ] {
            assert!(!kind.find("foo", source), "{source:?}");
            assert!(!kind.find_ascii_insensitive("FOO", source), "{source:?}");
        }
        assert!(kind.find("bar\u{00A0}foo", "baz bar\u{00A0}foo"));
    }

    #[test]
    fn whitespace_separated_ignores_empty_words() {
        let kind = AttributeSelectionKind::WhitespaceSeparated;
        for source in ["", "   ", "foo  bar", " foo bar "] {
            assert!(!kind.find("", source), "{source:?}");
            assert!(!kind.find_ascii_insensitive("", source), "{source:?}");
        }
    }

    #[test]
    fn test_prefix() {
        let kind = AttributeSelectionKind::Prefix;
        assert!(kind.find("hello wor", "hello world in test"));
    }

    #[test]
    fn test_with_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(kind.find("en", "en"));
        assert!(kind.find("en", "en-US"));
        assert!(kind.find("en", "en-us"));
    }

    #[test]
    fn test_without_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(!kind.find("en", "xx en-US"));
        assert!(!kind.find("en", "xen-US"));
        assert!(!kind.find("en", "hello en-world"));
        assert!(!kind.find("en", "hello en world"));
    }

    #[test]
    fn test_suffix() {
        let kind = AttributeSelectionKind::Suffix;
        assert!(kind.find("ld in test", "hello world in test"));
    }

    #[test]
    fn test_substring() {
        let kind = AttributeSelectionKind::Substring;
        assert!(kind.find("world", "helloworldintest"));
    }

    #[test]
    fn test_prefix_unicode_no_panic() {
        let kind = AttributeSelectionKind::Prefix;
        assert!(!kind.find("e", "éclair"));
    }

    #[test]
    fn test_suffix_unicode_no_panic() {
        let kind = AttributeSelectionKind::Suffix;
        assert!(!kind.find("e", "café"));
    }

    #[test]
    fn test_hyphen_separated_unicode_no_panic() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(!kind.find("e", "é-fr"));
    }

    #[test]
    fn empty_substring_operator_values_match_nothing() {
        for kind in [
            AttributeSelectionKind::Prefix,
            AttributeSelectionKind::Suffix,
            AttributeSelectionKind::Substring,
        ] {
            assert!(!kind.find("", "value"));
            assert!(!kind.find_ascii_insensitive("", "value"));
        }
    }
}
