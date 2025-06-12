use crate::utils::reader::Reader;

#[derive(Debug, PartialEq)]
pub enum QuoteKind {
    DoubleQuoted,
    SingleQuoted
}

#[derive(Debug, PartialEq)]
pub enum ElementAttributeToken<'a> {
    String(&'a str),
    Quote(QuoteKind),
    Equal,
}

impl<'a> ElementAttributeToken<'a> {
    pub fn next(reader: &mut Reader<'a>)  -> Option<Self> {
        reader.next_while(|c| c.is_whitespace());

        let start_pos = reader.get_position();

        return match reader.next()? {
            '"' => Some(Self::Quote(QuoteKind::DoubleQuoted)),
            '\'' => Some(Self::Quote(QuoteKind::SingleQuoted)),
            '=' => Some(Self::Equal),
            _ => {
                // Find end of word
                reader.next_while(|c| !c.is_whitespace() && c != '"' && c != '\'' && c != '=');
                return Some(Self::String(&reader.slice(start_pos..reader.get_position())));
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_attribute_iterator() {
        let string: String = String::from("key=\"value\"");

        let mut reader = Reader::new(&string);

        let mut next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        let mut next_value = next_iter.unwrap();

        assert_eq!(next_value, ElementAttributeToken::String("key"));

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::Equal);

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::Quote(QuoteKind::DoubleQuoted));

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::String("value"));

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::Quote(QuoteKind::DoubleQuoted));

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(!next_iter.is_some());
    }

    // TOKENIZER / FSM attribute robustness tests
    // TODO: key="value's" <-- `'` should be part of the string
    // TODO: k'ey="value" <-- `'` should be part of the string
    // TODO: key="v"alue" <-- parsed as `key="v"` and `alue"` which is equal to true.
}
