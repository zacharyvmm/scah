use super::tokenizer::{AttributeTokenIter, TokenKind};
use crate::utils::reader::{Reader, ReaderRef};

// This is the intended output of the all the nodes
// This exists if their is not hooks
struct GlobalAttribute {
    // list of classes
    // list of id's
}

struct Hook {
    // map of css selectors with a linked list of values
}

// This is the intended output of the node itself
struct Attribute {}

enum AttributeFsmState {
    NEW_KEY,
    ASSIGN_VALUE,
}

struct Pair<'a> {
    state: AttributeFsmState,
    key_buf: Option<&'a str>,

    // Key Value pair. The Value can be null
    pairs: Vec<(&'a str, Option<&'a str>)>,
}

impl<'a> Pair<'a> {
    fn new() -> Self {
        return Pair {
            state: AttributeFsmState::NEW_KEY,
            key_buf: None,
            pairs: Vec::new(),
        };
    }

    pub fn set_to_new_key(&mut self) -> () {
        if let Some(key) = self.key_buf {
            self.pairs.push((key, None));
            self.key_buf = None;
        }

        self.state = AttributeFsmState::NEW_KEY;
    }

    pub fn add_string(&mut self, content: &'a str) {
        match self.state {
            AttributeFsmState::NEW_KEY => {
                // This should not happen, but better safe then sorry
                // self.set_to_new_key();

                self.key_buf = Some(content);

                self.state = AttributeFsmState::ASSIGN_VALUE;
            }

            AttributeFsmState::ASSIGN_VALUE => {
                if let Some(key) = self.key_buf {
                    self.pairs.push((key, Some(content)));
                    self.key_buf = None;
                }

                self.state = AttributeFsmState::NEW_KEY;
            }
        }
    }
}

struct AttributeParser<'a> {
    iter: AttributeTokenIter<'a>,
    pair: Pair<'a>,
    reader: ReaderRef<'a>,
}

impl<'a> AttributeParser<'a> {
    fn new(reader: ReaderRef<'a>) -> Self {
        return AttributeParser {
            iter: AttributeTokenIter::new(reader.clone()),
            reader: reader,
            pair: Pair::new(),
        };
    }

    pub fn parse(&mut self) {
        let mut opened_quote = false;
        let mut position = 0;

        //for token in self.iter {
        while let Some(token) = self.iter.next() {
            let reader = self.reader.borrow();
            match (opened_quote, token.kind) {
                (false, TokenKind::QUOTE) => {
                    opened_quote = true;
                    position = reader.get_position();
                }

                (true, TokenKind::QUOTE) => {
                    opened_quote = false;

                    let end_position = reader.get_position() - token.value.len();
                    let content_inside_quotes = &reader.slice(position..end_position);

                    self.pair.add_string(content_inside_quotes);
                }

                (false, TokenKind::STRING) => {
                    self.pair.add_string(token.value);
                }

                (_, _) => (), //(false, TokenKind::EQUAL) => {
                              // Trust the fsm should work without this
                              //}
            }
        }

        self.pair.set_to_new_key();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_no_quote_and_value_with_quote() {
        let mut reader = Reader::new("key=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_no_quote_and_value_no_quote() {
        let mut reader = Reader::new("key=value");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_with_quote_and_value_with_quote() {
        let mut reader = Reader::new("\"key\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", Some("value")));
    }

    #[test]
    fn test_multiple_key_value_pairs() {
        let mut reader = Reader::new("key=\"value\" \"key1\"=value1 \"key2\"=\"value2\" keey");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", Some("value")));
        assert_eq!(parser.pair.pairs[1], ("key1", Some("value1")));
        assert_eq!(parser.pair.pairs[2], ("key2", Some("value2")));
        assert_eq!(parser.pair.pairs[3], ("keey", None));
    }

    #[test]
    fn test_key_with_quote_and_no_value() {
        let mut reader = Reader::new("\"key\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", None));
    }

    #[test]
    fn test_key_no_quote_and_no_value() {
        let mut reader = Reader::new("key");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.pairs[0], ("key", None));
    }

    #[test]
    fn test_long_key_with_spaces() {
        let mut reader = Reader::new("\"long key with spaces\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.pairs[0],
            ("long key with spaces", Some("value"))
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_different_quote_inside() {
        let mut reader = Reader::new("\"long key's with spaces\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.pairs[0],
            ("long key's with spaces", Some("value"))
        );
    }
}
