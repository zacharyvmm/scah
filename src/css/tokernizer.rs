use crate::utils::reader::Reader;

/*
* A css selector is made up of element specifiers.
* Element specifier (we know it's part of the element specification because their is no combiners in between):
* - type
* - attributes
* - class
* - id
*/

#[derive(Debug, PartialEq)]
pub enum ElementKind {}

pub struct CssSelectorTokenizer {
    reader: &Reader,
}

impl CssSelectorTokenizer {
    pub fn new(reader: &Reader) -> Self {
        return CssSelectorTokenizer { reader: reader };
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
        assert_eq!(false, true);
    }
}
