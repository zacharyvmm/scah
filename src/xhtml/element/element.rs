use super::pair::Pair;
use super::tokenizer::ElementAttributeToken;
use crate::utils::QuoteKind;
use crate::utils::Reader;

#[derive(Debug, PartialEq, Clone)]
pub struct Attribute<'a> {
    pub name: &'a str,
    pub value: Option<&'a str>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct XHtmlElement<'a> {
    pub name: &'a str,
    pub id: Option<&'a str>,
    pub class: Option<&'a str>,
    pub attributes: Vec<Attribute<'a>>,
}

#[derive(Debug, PartialEq)]
pub enum XHtmlTag<'a> {
    Open(XHtmlElement<'a>),
    Close(&'a str),
}

impl<'a> XHtmlElement<'a> {
    fn add_to_element(&mut self, attribute: Attribute<'a>) -> () {
        if self.name == "" && attribute.value.is_none() {
            self.name = attribute.name;
        } else if self.class.is_none() && attribute.name == "class" && attribute.value.is_some() {
            self.class = attribute.value;
        } else if self.id.is_none() && attribute.name == "id" && attribute.value.is_some() {
            self.id = attribute.value;
        } else {
            self.attributes.push(attribute);
        }
    }

    pub fn is_self_closing(&self) -> bool {
        if matches!(
            self.name,
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        ) {
            return true;
        }
        if let Some(last_attribute) = self.attributes.last() {
            return last_attribute.name == "\\";
        }

        return false;
    }
}

impl<'a> From<&mut Reader<'a>> for XHtmlElement<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut element = Self {
            name: "",
            id: None,
            class: None,
            attributes: Vec::new(),
        };

        let mut pair = Pair::NewKey;

        let mut opened_quote: Option<QuoteKind> = None;
        let mut position = reader.get_position();

        // TODO: I need to refactor this, this is not clean at all
        while let Some(token) = {
            let t = ElementAttributeToken::next(reader);
            if let Some(ElementAttributeToken::CloseElement) = t
                && opened_quote.is_none()
            {
                None
            } else {
                t
            }
        } {
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

                    if let Some(attribute) = pair.add_string(content_inside_quotes) {
                        element.add_to_element(attribute);
                    }
                }

                (Option::None, ElementAttributeToken::String(string_value)) => {
                    if let Some(attribute) = pair.add_string(string_value) {
                        element.add_to_element(attribute);
                    }
                }

                (_, ElementAttributeToken::Equal) => {
                    pair.set_assign_value();
                }

                (_, _) => (),
            }
        }

        if let Some(attribute) = pair.get_final_equal_value() {
            element.add_to_element(attribute);
        }

        return element;
    }
}

// TODO: Parse the closing tag for the XHtmlTag
impl<'a> XHtmlTag<'a> {
    pub fn from(reader: &mut Reader<'a>) -> Option<Self> {
        reader.next_while(|c| c.is_ascii_whitespace() || c == b'<');
        if let Some(character) = reader.peek() {
            if character == b'/' {
                let start = reader.get_position() + 1;
                reader.next_while(|c| c != b'>');

                let end = reader.get_position();
                reader.skip();

                // BUG: Handle start and end not conforming to the rules of slices.

                // BUG: The Formating of the string breaks this code

                return Some(Self::Close(reader.slice(start..end).trim()));
            } else if character == b'!' {
                // This is a comment
                reader.next_while(|c| c != b'>');
                reader.skip();
                return None;
            }
        }
        return Some(Self::Open(XHtmlElement::from(reader)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_no_quote_and_value_with_quote() {
        let mut reader = Reader::new("p key=\"value\"");
        let element = XHtmlElement::from(&mut reader);
        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_value_no_quote() {
        let mut reader = Reader::new("p key=value");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(element.attributes.len(), 1);

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_key_with_quote_and_value_with_quote() {
        let mut reader = Reader::new("p \"key\"=\"value\"");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_multiple_key_value_pairs() {
        let mut reader = Reader::new("p key=\"value\" \"key1\"=value1 \"key2\"=\"value2\" keey");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: Some("value")
            }
        );
        assert_eq!(
            element.attributes[1],
            Attribute {
                name: "key1",
                value: Some("value1")
            }
        );
        assert_eq!(
            element.attributes[2],
            Attribute {
                name: "key2",
                value: Some("value2")
            }
        );
        assert_eq!(
            element.attributes[3],
            Attribute {
                name: "keey",
                value: None
            }
        );
    }

    #[test]
    fn test_key_with_quote_and_no_value() {
        let mut reader = Reader::new("p \"key\"");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: None
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_no_value() {
        let mut reader = Reader::new("p key");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: None
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_escaped_space_value() {
        let mut reader = Reader::new("p key = hello\\ world");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "key",
                value: Some("hello\\ world")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces() {
        let mut reader = Reader::new("p \"long key with spaces\"=\"value\"");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "long key with spaces",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_different_quote_inside() {
        let mut reader = Reader::new("p \"long key's with spaces\"=\"value\"");
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "long key's with spaces",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_real_same_quote_inside() {
        let mut reader = Reader::new(r#"p "long key\"s with spaces"="value""#);
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: r#"long key\"s with spaces"#,
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_and_value_with_spaces_and_real_same_quote_inside() {
        let mut reader = Reader::new(
            r#"p "long key\"s with spaces"="value\"s of an other person \\\\\\ \\\\\ \ \  \"""#,
        );
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: r#"long key\"s with spaces"#,
                value: Some(r#"value\"s of an other person \\\\\\ \\\\\ \ \  \""#)
            }
        );
    }

    #[test]
    fn test_valid_anchor_tag_attributes() {
        let mut reader = Reader::new(
            "a target=\"_blank\" href=\"/my_cv.pdf\" class=\"px-7 py-3\" hello-world=hello-world",
        );
        let element = XHtmlElement::from(&mut reader);

        assert_eq!(element.name, "a");

        assert_eq!(
            element.attributes[0],
            Attribute {
                name: "target",
                value: Some("_blank")
            }
        );

        assert_eq!(
            element.attributes[1],
            Attribute {
                name: "href",
                value: Some("/my_cv.pdf")
            }
        );

        assert_eq!(element.class, Some("px-7 py-3"));

        assert_eq!(
            element.attributes[2],
            Attribute {
                name: "hello-world",
                value: Some("hello-world")
            }
        );
    }

    #[test]
    fn test_complex_open_tag() {
        let mut reader = Reader::new(
            r#"a href="https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/crossorigin" title="The crossorigin attribute, valid on the <audio>, <img>, <link>, <script>, and <video> elements, provides support for CORS, defining how the element handles cross-origin requests, thereby enabling the configuration of the CORS requests for the element's fetched data. Depending on the element, the attribute can be a CORS settings attribute.""#,
        );

        let tag = XHtmlTag::from(&mut reader);

        assert_eq!(
            tag,
            Some(XHtmlTag::Open(XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([
                    Attribute {
                        name: "href",
                        value: Some(
                            "https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/crossorigin"
                        )
                    },
                    Attribute {
                        name: "title",
                        value: Some(
                            "The crossorigin attribute, valid on the <audio>, <img>, <link>, <script>, and <video> elements, provides support for CORS, defining how the element handles cross-origin requests, thereby enabling the configuration of the CORS requests for the element's fetched data. Depending on the element, the attribute can be a CORS settings attribute."
                        )
                    }
                ]),
            }))
        );
    }

    #[test]
    fn test_xhtml_tag_open() {
        let mut reader = Reader::new("p key=\"value\"");
        let tag = XHtmlTag::from(&mut reader);

        assert_eq!(
            tag,
            Some(XHtmlTag::Open(XHtmlElement {
                name: "p",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "key",
                    value: Some("value")
                }]),
            }))
        );
    }

    #[test]
    fn test_xhtml_tag_close() {
        let mut reader = Reader::new("/p>");
        let tag = XHtmlTag::from(&mut reader);

        assert_eq!(tag, Some(XHtmlTag::Close("p")));
    }

    #[test]
    fn test_xhtml_tag_close_weird_formatting() {
        let mut reader = Reader::new("  /   p   >");
        let tag = XHtmlTag::from(&mut reader);

        assert_eq!(tag, Some(XHtmlTag::Close("p")));
    }
}
