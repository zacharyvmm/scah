use super::element::{AttributeSelection, QueryElement};
use super::string_search::AttributeSelectionKind;
//use crate::xhtml::element::element::{Attribute, XHtmlElement};
use crate::runner::element::{Attribute, XHtmlElement};

macro_rules! bytes_to_string_unsafe {
    ( $x:expr ) => {{ unsafe { str::from_utf8_unchecked($x) } }};
}

impl<'a, 'b> PartialEq<Attribute<'b>> for AttributeSelection<'a> {
    fn eq(&self, other: &Attribute<'b>) -> bool {
        if self.name != bytes_to_string_unsafe!(other.key) {
            return false;
        }

        if self.value.is_none() || self.kind == AttributeSelectionKind::Presence {
            return true;
        }

        if other.value.is_none() {
            return false;
        }

        return self.kind.find(
            self.value.unwrap(),
            bytes_to_string_unsafe!(other.value.unwrap()),
        );
    }
}

impl<'a, 'b> PartialEq<XHtmlElement<'b>> for QueryElement<'a> {
    fn eq(&self, other: &XHtmlElement<'b>) -> bool {
        if let Some(name) = self.name
            && name != bytes_to_string_unsafe!(other.name)
        {
            return false;
        }

        if self.id.is_some() && self.id.unwrap() != bytes_to_string_unsafe!(other.id.unwrap()) {
            return false;
        }

        if self.class.is_some()
            && (other.class.is_none()
                || !bytes_to_string_unsafe!(other.class.unwrap())
                    .split_whitespace()
                    .any(|word| word == self.class.unwrap()))
        {
            return false;
        }

        let other_attributes_conform_to_selector =
            !self.attributes.iter().all(|selector_attribute| {
                other
                    .attributes
                    .iter()
                    .any(|xhtml_attribute| selector_attribute == xhtml_attribute)
            });
        if other_attributes_conform_to_selector {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_selection_comparison() {
        assert_eq!(
            AttributeSelection {
                name: "hello",
                value: Some("World"),
                kind: AttributeSelectionKind::Exact,
            },
            Attribute {
                key: b"hello",
                value: Some(b"World")
            }
        );
    }

    #[test]
    fn test_element_selection_comparison() {
        assert_eq!(
            QueryElement {
                name: Some("hello"),
                id: Some("id"),
                class: Some("world"),
                attributes: Vec::from([AttributeSelection {
                    name: "selected",
                    value: Some("true"),
                    kind: AttributeSelectionKind::Exact
                }])
            },
            XHtmlElement {
                closing: false,
                name: b"hello",
                id: Some(b"id"),
                class: Some(b"hello world"),
                attributes: Vec::from([
                    Attribute {
                        key: b"key1",
                        value: Some(b"value1")
                    },
                    Attribute {
                        key: b"key2",
                        value: Some(b"value2")
                    },
                    Attribute {
                        key: b"selected",
                        value: Some(b"true")
                    },
                ])
            }
        );
    }

    #[test]
    fn test_realistic_search() {
        assert_eq!(
            QueryElement {
                name: Some("a"),
                id: None,
                class: Some("underline-green"),
                attributes: Vec::from([AttributeSelection {
                    name: "href",
                    value: None,
                    kind: AttributeSelectionKind::Presence,
                }])
            },
            XHtmlElement {
                closing: false,
                name: b"a",
                id: Some(b"search-link"),
                class: Some(b"text-white underline-green p-4"),
                attributes: Vec::from([
                    Attribute {
                        key: b"key1",
                        value: Some(b"value1")
                    },
                    Attribute {
                        key: b"href",
                        value: Some(b"/search")
                    },
                    Attribute {
                        key: b"selected",
                        value: Some(b"true")
                    },
                ])
            }
        );
    }
}
