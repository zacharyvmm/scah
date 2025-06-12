use crate::utils::reader::Reader;

#[derive(Debug, PartialEq)]
pub enum ElementAttributeToken<'a> {
    String(&'a str),
    Quote,
    Equal,
}

impl<'a> ElementAttributeToken<'a> {
    fn skip_whitespace(reader: &mut Reader) {
        while let Some(c) = reader.peek() {
            if c.is_whitespace() {
                reader.next();
            } else {
                break;
            }
        }
    }

    pub fn next(reader: &mut Reader<'a>)  -> Option<Self> {
        ElementAttributeToken::skip_whitespace(reader);


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
                return Some(Self::String(&reader.slice(start_pos..reader.get_position())));
            }
            '=' => Some(Self::Equal),
            '"' => Some(Self::Quote),
            '\'' => Some(Self::Quote),
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
        assert_eq!(next_value, ElementAttributeToken::Quote);

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::String("value"));

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(next_iter.is_some());

        next_value = next_iter.unwrap();
        assert_eq!(next_value, ElementAttributeToken::Quote);

        // -----
        next_iter = ElementAttributeToken::next(&mut reader);
        assert!(!next_iter.is_some());
    }

    // TOKENIZER / FSM attribute robustness tests
    // TODO: key="value's" <-- `'` should be part of the string
    // TODO: k'ey="value" <-- `'` should be part of the string
    // TODO: key="v"alue" <-- parsed as `key="v"` and `alue"` which is equal to true.
}
