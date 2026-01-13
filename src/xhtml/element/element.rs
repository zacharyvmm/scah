use super::pair::Pair;
use super::tokenizer::ElementAttributeToken;
use crate::utils::QuoteKind;
use crate::utils::Reader;
use smallvec::SmallVec;

#[derive(Debug, PartialEq, Clone)]
pub struct Attribute<'html> {
    pub key: &'html str,
    pub value: Option<&'html str>,
}

pub type Attributes<'html> = Vec<Attribute<'html>>;
//pub type Attributes<'html> = SmallVec<[Attribute<'html>, 3]>;

#[derive(Debug, PartialEq, Clone)]
pub struct XHtmlElement<'html> {
    pub name: &'html str,
    pub id: Option<&'html str>,
    pub class: Option<&'html str>,
    pub attributes: Attributes<'html>,
}

#[derive(Debug, PartialEq)]
pub enum XHtmlTag<'a> {
    Open,
    Close(&'a str),
}

impl<'a> XHtmlElement<'a> {
    fn add_to_element(&mut self, attribute: Attribute<'a>) {
        if self.name.is_empty() && attribute.value.is_none() {
            self.name = attribute.key;
        } else if self.class.is_none() && attribute.key == "class" && attribute.value.is_some() {
            self.class = attribute.value;
        } else if self.id.is_none() && attribute.key == "id" && attribute.value.is_some() {
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
            return last_attribute.key == "\\";
        }

        return false;
    }

    pub fn new() -> Self {
        Self {
            name: "",
            id: None,
            class: None,
            attributes: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.name = "";
        self.id = None;
        self.class = None;
        self.attributes.clear();
    }

    pub fn from(&mut self, reader: &mut Reader<'a>) {
        let mut assign = false;
        let mut key = None;

        while let Some(token) = ElementAttributeToken::next(reader) {
            match token {
                ElementAttributeToken::String(string_value) => match key {
                    None => {
                        debug_assert!(!assign);
                        key = Some(string_value);
                    }
                    Some(k) => {
                        if assign {
                            self.add_to_element(Attribute {
                                key: k,
                                value: Some(string_value),
                            });
                            key = None;
                        } else {
                            self.add_to_element(Attribute {
                                key: k,
                                value: None,
                            });
                            key = Some(string_value)
                        }
                        assign = false;
                    }
                },

                ElementAttributeToken::Equal => {
                    assign = true;
                }
            }
        }

        if let Some(attribute) = key {
            self.add_to_element(Attribute {
                key: attribute,
                value: None,
            });
        }
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
        return Some(Self::Open);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_no_quote_and_value_with_quote() {
        let mut reader = Reader::new("p key=\"value\"");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);
        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_value_no_quote() {
        let mut reader = Reader::new("p key=value");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(element.attributes.len(), 1);

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_key_with_quote_and_value_with_quote() {
        let mut reader = Reader::new("p \"key\"=\"value\"");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_multiple_key_value_pairs() {
        let mut reader = Reader::new("p key=\"value\" \"key1\"=value1 \"key2\"=\"value2\" keey");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: Some("value")
            }
        );
        assert_eq!(
            element.attributes[1],
            Attribute {
                key: "key1",
                value: Some("value1")
            }
        );
        assert_eq!(
            element.attributes[2],
            Attribute {
                key: "key2",
                value: Some("value2")
            }
        );
        assert_eq!(
            element.attributes[3],
            Attribute {
                key: "keey",
                value: None
            }
        );
    }

    #[test]
    fn test_key_with_quote_and_no_value() {
        let mut reader = Reader::new("p \"key\"");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: None
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_no_value() {
        let mut reader = Reader::new("p key");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: None
            }
        );
    }

    #[test]
    fn test_key_no_quote_and_escaped_space_value() {
        let mut reader = Reader::new("p key = hello\\ world");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "key",
                value: Some("hello\\ world")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces() {
        let mut reader = Reader::new("p \"long key with spaces\"=\"value\"");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "long key with spaces",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_different_quote_inside() {
        let mut reader = Reader::new("p \"long key's with spaces\"=\"value\"");
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "long key's with spaces",
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_with_spaces_and_real_same_quote_inside() {
        let mut reader = Reader::new(r#"p "long key\"s with spaces"="value""#);
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: r#"long key\"s with spaces"#,
                value: Some("value")
            }
        );
    }

    #[test]
    fn test_long_key_and_value_with_spaces_and_real_same_quote_inside() {
        let mut reader = Reader::new(
            r#"p "long key\"s with spaces"="value\"s of an other person \\\\\\ \\\\\ \ \  \"""#,
        );
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "p");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: r#"long key\"s with spaces"#,
                value: Some(r#"value\"s of an other person \\\\\\ \\\\\ \ \  \""#)
            }
        );
    }

    #[test]
    fn test_valid_anchor_tag_attributes() {
        let mut reader = Reader::new(
            "a target=\"_blank\" href=\"/my_cv.pdf\" class=\"px-7 py-3\" hello-world=hello-world",
        );
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(element.name, "a");

        assert_eq!(
            element.attributes[0],
            Attribute {
                key: "target",
                value: Some("_blank")
            }
        );

        assert_eq!(
            element.attributes[1],
            Attribute {
                key: "href",
                value: Some("/my_cv.pdf")
            }
        );

        assert_eq!(element.class, Some("px-7 py-3"));

        assert_eq!(
            element.attributes[2],
            Attribute {
                key: "hello-world",
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
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(tag, Some(XHtmlTag::Open));

        assert_eq!(
            element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([
                    Attribute {
                        key: "href",
                        value: Some(
                            "https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/crossorigin"
                        )
                    },
                    Attribute {
                        key: "title",
                        value: Some(
                            "The crossorigin attribute, valid on the <audio>, <img>, <link>, <script>, and <video> elements, provides support for CORS, defining how the element handles cross-origin requests, thereby enabling the configuration of the CORS requests for the element's fetched data. Depending on the element, the attribute can be a CORS settings attribute."
                        )
                    }
                ]),
            }
        );
    }

    #[test]
    fn test_xhtml_tag_open() {
        let mut reader = Reader::new("p key=\"value\"");
        let tag = XHtmlTag::from(&mut reader);
        let mut element = XHtmlElement::new();
        element.from(&mut reader);

        assert_eq!(tag, Some(XHtmlTag::Open));

        assert_eq!(
            element,
            XHtmlElement {
                name: "p",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    key: "key",
                    value: Some("value")
                }]),
            }
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
