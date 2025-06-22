#[derive(Debug, PartialEq)]
pub enum AttributeSelectionKind {
    Presence,            // [attribute]
    Exact,               // [attribute=value]
    WhitespaceSeparated, // [attribute~=value]
    HyphenSeparated,     // [attribute|=value]
    Prefix,              // [attribute^=value]
    Suffix,              // [attribute$=value]
    Substring,           // [attribute*=value]
}

impl AttributeSelectionKind {
    pub fn find<'a, 'b>(&self, query: &'a str, source: &'b str) -> bool {
        match self {
            Self::Exact => query == source,
            Self::Presence => true,
            Self::WhitespaceSeparated => source.split_whitespace().any(|word| word == query),
            Self::HyphenSeparated => source.split_whitespace().any(|word| {
                if  word == query {
                    return true;
                }

                if query.len() + 1 > word.len() {
                    return false;
                }


                // query is prefix of word with `-` nextup
                return query == &word[0..query.len()] && "-" == &word[query.len()..query.len()+1];
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

            Self::Substring => {
                source.contains(query)
            }
        }
    }
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
    fn test_with_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated; 
        assert!(kind.find("en", "hello en-world"));
    }

    #[test]
    fn test_without_hypen_separated() {
        let kind = AttributeSelectionKind::HyphenSeparated;
        assert!(kind.find("en", "hello en world"));
    }

    #[test]
    fn test_prefix() {
        let kind = AttributeSelectionKind::Prefix;
        assert!(kind.find("hello wor", "hello world in test"));
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
}