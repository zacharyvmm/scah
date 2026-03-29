use super::builder::{Attribute, AttributeSelection, ElementPredicate, IElement};
use super::string_search::AttributeSelectionKind;

impl<'a> AttributeSelection<'a> {
    pub fn matches_attribute(&self, other: &Attribute<'_>) -> bool {
        if self.name != other.key {
            return false;
        }

        if self.value.is_none() || self.kind == AttributeSelectionKind::Presence {
            return true;
        }

        if other.value.is_none() {
            return false;
        }

        self.kind.find(self.value.unwrap(), other.value.unwrap())
    }
}

impl<'a> ElementPredicate<'a> {
    pub fn matches_element<'b, E: IElement<'b>>(&self, other: &E) -> bool {
        if let Some(name) = self.name
            && name != other.name()
        {
            return false;
        }

        if self.id.is_some() && self.id != other.id() {
            return false;
        }

        if self.class.is_some()
            && (other.class().is_none()
                || !other
                    .class()
                    .unwrap()
                    .split_whitespace()
                    .any(|word| word == self.class.unwrap()))
        {
            return false;
        }

        self.attributes.as_slice().iter().all(|selector_attribute| {
            other
                .attributes()
                .iter()
                .any(|xhtml_attribute| selector_attribute.matches_attribute(xhtml_attribute))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AttributeSelections;

    #[derive(Debug)]
    struct FakeElement<'a> {
        name: &'a str,
        id: Option<&'a str>,
        class: Option<&'a str>,
        attributes: &'a [Attribute<'a>],
    }

    impl<'a> IElement<'a> for FakeElement<'a> {
        fn name(&self) -> &'a str {
            self.name
        }

        fn id(&self) -> Option<&'a str> {
            self.id
        }

        fn class(&self) -> Option<&'a str> {
            self.class
        }

        fn attributes(&self) -> &[Attribute<'a>] {
            self.attributes
        }
    }

    #[test]
    fn test_attribute_selection_comparison() {
        assert!(
            AttributeSelection {
                name: "hello",
                value: Some("World"),
                kind: AttributeSelectionKind::Exact,
            }
            .matches_attribute(&Attribute {
                key: "hello",
                value: Some("World")
            })
        );
    }

    #[test]
    fn test_element_selection_comparison() {
        assert!(
            ElementPredicate {
                name: Some("hello"),
                id: Some("id"),
                class: Some("world"),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "selected",
                    value: Some("true"),
                    kind: AttributeSelectionKind::Exact
                }])
            }
            .matches_element(&FakeElement {
                name: "hello",
                id: Some("id"),
                class: Some("hello world"),
                attributes: &[
                    Attribute {
                        key: "key1",
                        value: Some("value1")
                    },
                    Attribute {
                        key: "key2",
                        value: Some("value2")
                    },
                    Attribute {
                        key: "selected",
                        value: Some("true")
                    },
                ]
            })
        );
    }

    #[test]
    fn test_realistic_search() {
        assert!(
            ElementPredicate {
                name: Some("a"),
                id: None,
                class: Some("underline-green"),
                attributes: AttributeSelections::from(vec![AttributeSelection {
                    name: "href",
                    value: None,
                    kind: AttributeSelectionKind::Presence,
                }])
            }
            .matches_element(&FakeElement {
                name: "a",
                id: Some("search-link"),
                class: Some("text-white underline-green p-4"),
                attributes: &[
                    Attribute {
                        key: "key1",
                        value: Some("value1")
                    },
                    Attribute {
                        key: "href",
                        value: Some("/search")
                    },
                    Attribute {
                        key: "selected",
                        value: Some("true")
                    },
                ]
            })
        );
    }
}
