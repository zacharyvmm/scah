use super::tokenizer::{ElementAttributeToken, QuoteKind};
use crate::utils::reader::Reader;
use crate::utils::pair::Pair;


struct AttributeParser<'a> {
    pair: Pair<'a>,
    reader: Reader<'a>,
}

impl<'a> AttributeParser<'a> {
    fn new(reader: Reader<'a>) -> Self {
        return AttributeParser {
            reader: reader,
            pair: Pair::new(),
        };
    }

    pub fn parse(&mut self) {
        let mut opened_quote: Option<QuoteKind> = None;
        let mut position = self.reader.get_position();

        //for token in self.iter {
        while let Some(token) = ElementAttributeToken::next(&mut self.reader) {
            match (&opened_quote, token) {
                (Option::None, ElementAttributeToken::Quote(kind)) => {
                    opened_quote = Some(kind);
                    position = self.reader.get_position();
                }

                (Some(previous_quote), ElementAttributeToken::Quote(kind)) => {
                    if *previous_quote != kind {
                        continue;
                    }

                    opened_quote = None;

                    // `"` and `'` are always of size 1
                    const SIZE_OF_QUOTE:usize = 1;

                    let end_position = self.reader.get_position() - SIZE_OF_QUOTE;
                    let content_inside_quotes = self.reader.slice(position..end_position);

                    self.pair.add_string(content_inside_quotes);
                }

                (Option::None, ElementAttributeToken::String(string_value)) => {
                    self.pair.add_string(string_value);
                }

                (_, ElementAttributeToken::Equal) => {
                    self.pair.set_to_assign_value();
                }

                (_, _) => (),
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
        let reader = Reader::new("key=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_no_quote_and_value_no_quote() {
        let reader = Reader::new("key=value");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_with_quote_and_value_with_quote() {
        let reader = Reader::new("\"key\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_multiple_key_value_pairs() {
        let reader = Reader::new("key=\"value\" \"key1\"=value1 \"key2\"=\"value2\" keey");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
        assert_eq!(parser.pair.get_pairs()[1], ("key1", Some("value1")));
        assert_eq!(parser.pair.get_pairs()[2], ("key2", Some("value2")));
        assert_eq!(parser.pair.get_pairs()[3], ("keey", None));
    }

    #[test]
    fn test_key_with_quote_and_no_value() {
        let reader = Reader::new("\"key\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", None));
    }

    #[test]
    fn test_key_no_quote_and_no_value() {
        let reader = Reader::new("key");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(parser.pair.get_pairs()[0], ("key", None));
    }

    #[test]
    fn test_long_key_with_spaces() {
        let reader = Reader::new("\"long key with spaces\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key with spaces", Some("value"))
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_different_quote_inside() {
        let reader = Reader::new("\"long key's with spaces\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key's with spaces", Some("value"))
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_real_same_quote_inside() {
        let reader = Reader::new("\"long key\\\"s with spaces\"=\"value\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key\\\"s with spaces", Some("value"))
        );
    }

    #[test]
    fn test_valid_anchor_tag_attributes() {
        let reader = Reader::new("a target=\"_blank\" href=\"/my_cv.pdf\" class=\"px-7 py-3\"");
        let mut parser = AttributeParser::new(reader);
        parser.parse();

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("a", None)
        );

        assert_eq!(
            parser.pair.get_pairs()[1],
            ("target", Some("_blank"))
        );

        assert_eq!(
            parser.pair.get_pairs()[2],
            ("href", Some("/my_cv.pdf"))
        );

        assert_eq!(
            parser.pair.get_pairs()[3],
            ("class", Some("px-7 py-3"))
        );
    }
}
