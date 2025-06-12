use crate::utils::reader::Reader;

#[derive(Debug, PartialEq)]
pub enum TokenKind {
    STRING,
    QUOTE,
    EQUAL,
}

#[derive(Debug, PartialEq)]
pub struct AttributeToken<'a> {
    pub kind: TokenKind,
    pub value: &'a str,
}

pub struct AttributeTokenIter {
}

impl AttributeTokenIter {
    fn skip_whitespace(reader: &mut Reader) {
        while let Some(c) = reader.peek() {
            if c.is_whitespace() {
                reader.next();
            } else {
                break;
            }
        }
    }

    pub fn next<'a>(reader: &mut Reader<'a>) -> Option<AttributeToken<'a>> {
        Self::skip_whitespace(reader);

        let start_pos = reader.get_position();

        return match reader.next()? {
            c if c.is_alphabetic() => {
                // Find end of word
                while let Some(next) = reader.peek() {
                    if next.is_alphanumeric() {
                        reader.next();
                    } else {
                        break;
                    }
                }
                Some(AttributeToken {
                    kind: TokenKind::STRING,
                    value: &reader.slice(start_pos..reader.get_position()),
                })
            }
            '=' => Some(AttributeToken {
                kind: TokenKind::EQUAL,
                value: &reader.slice(start_pos..reader.get_position()),
            }),
            '"' => Some(AttributeToken {
                kind: TokenKind::QUOTE,
                value: &reader.slice(start_pos..reader.get_position()),
            }),
            '\'' => Some(AttributeToken {
                kind: TokenKind::QUOTE,
                value: &reader.slice(start_pos..reader.get_position()),
            }),
            _ => None,
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

        let mut next_iter = AttributeTokenIter::next(&mut reader);
        assert!(next_iter.is_some());

        let mut next_value = next_iter.unwrap();

        assert_eq!(next_value.kind, TokenKind::STRING);
        assert_eq!(next_value.value, "key");

        // -----
        next_iter = AttributeTokenIter::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::EQUAL);
        assert_eq!(next_value.value, "=");

        // -----
        next_iter = AttributeTokenIter::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::QUOTE);
        assert_eq!(next_value.value, "\"");

        // -----
        next_iter = AttributeTokenIter::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::STRING);
        assert_eq!(next_value.value, "value");

        // -----
        next_iter = AttributeTokenIter::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::QUOTE);
        assert_eq!(next_value.value, "\"");

        // -----
        next_iter = AttributeTokenIter::next(&mut reader);
        assert!(!next_iter.is_some());
    }

    // TOKENIZER / FSM attribute robustness tests
    // TODO: key="value's" <-- `'` should be part of the string
    // TODO: k'ey="value" <-- `'` should be part of the string
    // TODO: key="v"alue" <-- parsed as `key="v"` and `alue"` which is equal to true.
}
