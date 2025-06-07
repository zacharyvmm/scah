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
    position: usize,
    source: &'a str,
}

impl<'a> AttributeTokenIter<'a> {
    pub fn new(start: usize, input: &'a str) -> Self {
        return AttributeTokenIter {
            position: start,
            source: input,
        };
    }

    fn peek_char(&self) -> Option<char> {
        // This transformation should probably be done before hand
        return self.source[self.position..].chars().next();
    }

    fn consume_char(&mut self) -> Option<char> {
        let next_char = self.peek_char()?;
        self.position += next_char.len_utf8();
        return Some(next_char);
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.consume_char();
            } else {
                break;
            }
        }
    }

    pub fn current_position(&self) -> usize {
        return self.position;
    }

    fn create_token(&self, start: usize, end: usize, kind: TokenKind) -> AttributeToken<'a> {
        return AttributeToken {
            kind,
            value: &self.source[start..end],
        };
    }
}

impl<'a> Iterator for AttributeTokenIter<'a> {
    type Item = AttributeToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();
        let start_pos = self.current_position();

        return match self.consume_char()? {
            c if c.is_alphabetic() => {
                // Find end of word
                while let Some(next) = self.peek_char() {
                    if next.is_alphanumeric() {
                        self.consume_char();
                    } else {
                        break;
                    }
                }
                Some(self.create_token(start_pos, self.position, TokenKind::STRING))
            }
            '=' => Some(self.create_token(start_pos, self.position, TokenKind::EQUAL)),
            '"' => Some(self.create_token(start_pos, self.position, TokenKind::QUOTE)),
            '\'' => Some(self.create_token(start_pos, self.position, TokenKind::QUOTE)),
            _ => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_attribute_iterator() {
        let mut iterator = AttributeTokenIter::new(0, "key=\"value\"");

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
