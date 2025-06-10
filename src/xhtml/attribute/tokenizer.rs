use crate::utils::reader::{Reader, ReaderRef};

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

pub struct AttributeTokenIter<'a> {
    reader: ReaderRef<'a>,
}

impl<'a> AttributeTokenIter<'a> {
    pub fn new(reader: ReaderRef<'a>) -> Self {
        return AttributeTokenIter { reader: reader };
    }

    fn skip_whitespace(&mut self) {
        let mut reader = self.reader.borrow_mut();
        while let Some(c) = reader.peek() {
            if c.is_whitespace() {
                reader.next();
            } else {
                break;
            }
        }
    }
}

impl<'a> Iterator for AttributeTokenIter<'a> {
    type Item = AttributeToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();

        let mut reader = self.reader.borrow_mut();
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

        let mut iterator = AttributeTokenIter::new(reader);

        let mut next_iter = iterator.next();
        assert!(next_iter.is_some());

        let mut next_value = next_iter.unwrap();

        assert_eq!(next_value.kind, TokenKind::STRING);
        assert_eq!(next_value.value, "key");

        // -----
        next_iter = iterator.next();
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::EQUAL);
        assert_eq!(next_value.value, "=");

        // -----
        next_iter = iterator.next();
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::QUOTE);
        assert_eq!(next_value.value, "\"");

        // -----
        next_iter = iterator.next();
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::STRING);
        assert_eq!(next_value.value, "value");

        // -----
        next_iter = iterator.next();
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value.kind, TokenKind::QUOTE);
        assert_eq!(next_value.value, "\"");

        // -----
        next_iter = iterator.next();
        assert!(!next_iter.is_some());
    }

    // TOKENIZER / FSM attribute robustness tests
    // TODO: key="value's" <-- `'` should be part of the string
    // TODO: k'ey="value" <-- `'` should be part of the string
    // TODO: key="v"alue" <-- parsed as `key="v"` and `alue"` which is equal to true.
}
