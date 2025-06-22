use crate::{css::element::string_search::AttributeSelectionKind, xhtml::element::parser::{Attribute, XHtmlElement}};
use super::element::{Element, AttributeSelection};

impl<'a, 'b> PartialEq<Attribute<'b>> for AttributeSelection<'a> {
    fn eq(&self, other: &Attribute<'b>) -> bool {
        if self.name != other.name {
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

    /*
    #[inline]
    fn ne(&self, other: &Attribute<'b>) -> bool {
        !self.eq(other)
    }
    */
}

impl<'a, 'b> PartialEq<XHtmlElement<'b>> for Element<'a> {
    fn eq(&self, other: &XHtmlElement<'b>) -> bool {
        if self.name.is_some() && self.name != other.name {
            return false;
        }

        if self.id.is_some() && self.id != other.id {
            return false;
        }

        if self.class.is_some() && other.class.is_some() 
        && !other.class.unwrap().split_whitespace().any(|word| word == self.class.unwrap()) {
            return false;
        }

        let other_attributes_conform_to_selector = !self.attributes.iter().all(|selector_attribute| {
            other.attributes.iter().any(|xhtml_attribute| selector_attribute == xhtml_attribute)
        });
        if other_attributes_conform_to_selector {
            return false;
        }

        return true;
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
                name: "hello",
                value: Some("World")
            }
        );
    }

    #[test]
    fn test_element_selection_comparison() {
        assert_eq!(
            Element {
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
                name: Some("hello"),
                id: Some("id"),
                class: Some("hello world"),
                attributes: Vec::from([
                    Attribute {
                        name: "key1",
                        value: Some("value1")
                    },
                    Attribute {
                        name: "key2",
                        value: Some("value2")
                    },
                    Attribute {
                        name: "selected",
                        value: Some("true")
                    },
                ])
            }
        );
    }
}