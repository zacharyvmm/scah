use super::element::{AttributeSelection, QueryElement};
use super::string_search::{AttributeSelectionKind, split_whitespace_any};
//use crate::xhtml::element::element::{Attribute, XHtmlElement};
use crate::runner::element::{Attribute, XHtmlElement};

impl<'a, 'b> PartialEq<Attribute<'b>> for AttributeSelection<'a> {
    fn eq(&self, other: &Attribute<'b>) -> bool {
        if self.name != other.key {
            return false;
        }

        if self.value.is_none() || self.kind == AttributeSelectionKind::Presence {
            return true;
        }

        if other.value.is_none() {
            return false;
        }

        return self.kind.find(self.value.unwrap(), other.value.unwrap());
    }
}

impl<'a, 'b> PartialEq<XHtmlElement<'b>> for QueryElement<'a> {
    fn eq(&self, other: &XHtmlElement<'b>) -> bool {
        if let Some(name) = self.name
            && name != other.name
        {
            return false;
        }

        if self.id.is_some() && self.id.unwrap() != other.id.unwrap() {
            return false;
        }

        if self.class.is_some()
            && (other.class.is_none()
                || !split_whitespace_any(other.class.unwrap(), |word| word == self.class.unwrap()))
        {
            return false;
        }

        // let other_attributes_conform_to_selector =
        //     !self.attributes.iter().all(|selector_attribute| {
        //         other
        //             .attributes
        //             .iter()
        //             .any(|xhtml_attribute| selector_attribute == xhtml_attribute)
        //     });
        // if other_attributes_conform_to_selector {
        //     return false;
        // }
        if !self.attributes.is_empty() {
            if other.attributes.len() < self.attributes.len() {
                return false;
            }

            let mut all = true;
            for req_attr in &self.attributes {
                let mut any = false;
                for elem_attr in &other.attributes {
                    if req_attr == elem_attr {
                        any = true;
                        break;
                    }
                }
                all &= any;
            }
            return all;
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
                name: b"hello",
                value: Some(b"World"),
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
                name: Some(b"hello"),
                id: Some(b"id"),
                class: Some(b"world"),
                attributes: Vec::from([AttributeSelection {
                    name: b"selected",
                    value: Some(b"true"),
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
                name: Some(b"a"),
                id: None,
                class: Some(b"underline-green"),
                attributes: Vec::from([AttributeSelection {
                    name: b"href",
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
