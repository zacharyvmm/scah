use crate::utils::QuoteKind;
use crate::utils::Reader;

#[derive(Debug, PartialEq)]
pub enum ElementAttributeToken<'a> {
    String(&'a str),
    Equal,
}

const DOUBLEQUOTE: u8 = b'"';
const SINGLEQUOTE: u8 = b'\'';
const EQUAL: u8 = b'=';
const END_OF_ELEMENT: u8 = b'>';

impl<'a> ElementAttributeToken<'a> {
    pub fn next(reader: &mut Reader<'a>) -> Option<Self> {
        reader.next_while_char(b' ');

        let start_pos = reader.get_position();

        match reader.next()? {
            DOUBLEQUOTE => {
                let star_position = reader.get_position();
                reader.next_until(DOUBLEQUOTE);
                let content_inside_quotes = reader.slice(star_position..reader.get_position());
                reader.skip();

                Some(Self::String(content_inside_quotes))
            }
            SINGLEQUOTE => {
                let star_position = reader.get_position();
                reader.next_until(SINGLEQUOTE);
                let content_inside_quotes = reader.slice(star_position..reader.get_position());
                reader.skip();

                Some(Self::String(content_inside_quotes))
            }
            EQUAL => Some(Self::Equal),
            END_OF_ELEMENT => None,
            _ => {
                // Find end of word
                reader.next_until_char_list(&[
                    b' ',
                    DOUBLEQUOTE,
                    SINGLEQUOTE,
                    EQUAL,
                    END_OF_ELEMENT,
                ]);
                return Some(Self::String(reader.slice(start_pos..reader.get_position())));
            }
        }
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

        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::Equal);

        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::String("value"));
    }

    // TOKENIZER / FSM attribute robustness tests
    // TODO: key="value's" <-- `'` should be part of the string
    // TODO: k'ey="value" <-- `'` should be part of the string
    // TODO: key="v"alue" <-- parsed as `key="v"` and `alue"` which is equal to true.
}
