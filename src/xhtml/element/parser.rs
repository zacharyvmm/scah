use super::tokenizer::ElementAttributeToken;
use crate::utils::pair::Pair;
use crate::utils::reader::Reader;
use crate::utils::token::QuoteKind;

pub struct AttributeParser<'a> {
    // NOTE: Should I be using Pair or just creating a Element
    pair: Pair<'a>,
}

impl<'a> From<&mut Reader<'a>> for AttributeParser<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut parser = AttributeParser { pair: Pair::new() };

        let mut opened_quote: Option<QuoteKind> = None;
        let mut position = reader.get_position();

        //for token in self.iter {
        while let Some(token) = ElementAttributeToken::next(reader) {
            match (&opened_quote, token) {
                (Option::None, ElementAttributeToken::Quote(kind)) => {
                    opened_quote = Some(kind);
                    position = reader.get_position();
                }

                (Some(previous_quote), ElementAttributeToken::Quote(kind)) => {
                    if *previous_quote != kind {
                        continue;
                    }

                    opened_quote = None;

                    // `"` and `'` are always of size 1
                    const SIZE_OF_QUOTE: usize = 1;

                    let end_position = reader.get_position() - SIZE_OF_QUOTE;
                    let content_inside_quotes = reader.slice(position..end_position);

                    parser.pair.add_string(content_inside_quotes);
                }

                (Option::None, ElementAttributeToken::String(string_value)) => {
                    parser.pair.add_string(string_value);
                }

                (_, ElementAttributeToken::Equal) => {
                    parser.pair.set_to_assign_value();
                }

                (_, _) => (),
            }
        }

        parser.pair.set_to_new_key();

        return parser;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_no_quote_and_value_with_quote() {
        let mut reader = Reader::new("key=\"value\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_no_quote_and_value_no_quote() {
        let mut reader = Reader::new("key=value");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_key_with_quote_and_value_with_quote() {
        let mut reader = Reader::new("\"key\"=\"value\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
    }

    #[test]
    fn test_multiple_key_value_pairs() {
        let mut reader = Reader::new("key=\"value\" \"key1\"=value1 \"key2\"=\"value2\" keey");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", Some("value")));
        assert_eq!(parser.pair.get_pairs()[1], ("key1", Some("value1")));
        assert_eq!(parser.pair.get_pairs()[2], ("key2", Some("value2")));
        assert_eq!(parser.pair.get_pairs()[3], ("keey", None));
    }

    #[test]
    fn test_key_with_quote_and_no_value() {
        let mut reader = Reader::new("\"key\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", None));
    }

    #[test]
    fn test_key_no_quote_and_no_value() {
        let mut reader = Reader::new("key");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("key", None));
    }

    #[test]
    fn test_long_key_with_spaces() {
        let mut reader = Reader::new("\"long key with spaces\"=\"value\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key with spaces", Some("value"))
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_different_quote_inside() {
        let mut reader = Reader::new("\"long key's with spaces\"=\"value\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key's with spaces", Some("value"))
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_real_same_quote_inside() {
        let mut reader = Reader::new("\"long key\\\"s with spaces\"=\"value\"");
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(
            parser.pair.get_pairs()[0],
            ("long key\\\"s with spaces", Some("value"))
        );
    }

    #[test]
    fn test_valid_anchor_tag_attributes() {
        let mut reader = Reader::new(
            "a target=\"_blank\" href=\"/my_cv.pdf\" class=\"px-7 py-3\" hello-world=hello-world",
        );
        let parser = AttributeParser::from(&mut reader);

        assert_eq!(parser.pair.get_pairs()[0], ("a", None));

        assert_eq!(parser.pair.get_pairs()[1], ("target", Some("_blank")));

        assert_eq!(parser.pair.get_pairs()[2], ("href", Some("/my_cv.pdf")));

        assert_eq!(parser.pair.get_pairs()[3], ("class", Some("px-7 py-3")));

        assert_eq!(
            parser.pair.get_pairs()[4],
            ("hello-world", Some("hello-world"))
        );
    }
}
