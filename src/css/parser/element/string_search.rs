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

pub fn split_whitespace_any(string: &[u8], condition: impl Fn(&[u8]) -> bool) -> bool {
    let mut start = 0;
    let mut any = false;
    for (i, c) in string.iter().enumerate() {
        if *c == b' ' {
            any |= condition(&string[start..i]);
            start = i + 1;
        }
    }

    if start < string.len() {
        any |= condition(&string[start..string.len()]);
    }

    any
}

impl AttributeSelectionKind {
    pub fn find(&self, query: &[u8], source: &[u8]) -> bool {
        match self {
            Self::Exact => query == source,
            Self::Presence => true,
            Self::WhitespaceSeparated => split_whitespace_any(source, |word| word == query),
            Self::HyphenSeparated => split_whitespace_any(source, |word| {
                if word == query {
                    return true;
                }

                if query.len() + 1 > word.len() {
                    return false;
                }

                // query is prefix of word with `-` nextup
                return query == &word[0..query.len()] && b'-' == word[query.len()];
            }),
            Self::Prefix => {
                if query.len() > source.len() {
                    return false;
                }
                query == &source[0..query.len()]
            }
            Self::Suffix => {
                if query.len() > source.len() {
                    return false;
                }
                query == &source[(source.len() - query.len())..]
            }

            Self::Substring => unsafe { str::from_utf8_unchecked(source) }
                .contains(unsafe { str::from_utf8_unchecked(query) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presence() {
        let kind = AttributeSelectionKind::Presence;
        assert!(kind.find(b"", b"Hello"));
    }

    #[test]
    fn test_exact() {
        let kind = AttributeSelectionKind::Exact;
        assert!(kind.find(b"Hello", b"Hello"));
    }

    #[test]
    fn test_whitespace() {
        let kind = AttributeSelectionKind::WhitespaceSeparated;
        assert!(kind.find(b"world", b"hello world in test"));
    }

    #[test]
    fn test_with_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(kind.find(b"en", b"hello en-world"));
    }

    #[test]
    fn test_without_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(kind.find(b"en", b"hello en world"));
    }

    #[test]
    fn test_prefix() {
        let kind = AttributeSelectionKind::Prefix;
        assert!(kind.find(b"hello wor", b"hello world in test"));
    }

    #[test]
    fn test_suffix() {
        let kind = AttributeSelectionKind::Suffix;
        assert!(kind.find(b"ld in test", b"hello world in test"));
    }

    #[test]
    fn test_substring() {
        let kind = AttributeSelectionKind::Substring;
        assert!(kind.find(b"world", b"helloworldintest"));
    }
}
