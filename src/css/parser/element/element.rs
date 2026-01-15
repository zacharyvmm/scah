use super::string_search::AttributeSelectionKind;
use crate::utils::{QuoteKind, Reader};

#[derive(Debug, PartialEq, Clone)]
pub struct AttributeSelection<'query> {
    pub(super) name: &'query str,
    pub(super) value: Option<&'query str>,
    pub(super) kind: AttributeSelectionKind,
}

struct KeyValueAttributeSelection<'query> {
    name: Option<&'query str>,
    selection_kind: AttributeSelectionKind,
    value: Option<&'query str>,
}

impl<'query> KeyValueAttributeSelection<'query> {
    fn push(&mut self, content_inside_quotes: &'query str) {
        if self.name.is_none() {
            self.name = Some(content_inside_quotes);
        } else if self.value.is_none() {
            self.value = Some(content_inside_quotes);
        } else {
            unreachable!();
        }
    }

    fn refresh_equal(&mut self) {
        if self.selection_kind == AttributeSelectionKind::Presence && self.value.is_some() {
            self.selection_kind = AttributeSelectionKind::Exact;
        }
    }
}

impl<'query> From<&mut Reader<'query>> for AttributeSelection<'query> {
    fn from(reader: &mut Reader<'query>) -> Self {
        let mut position = reader.get_position();

        let mut opened_quote: Option<QuoteKind> = None;
        let mut equal = false;

        // let mut name: Option<&str> = None;
        // let mut value: Option<&str> = None;
        // let mut selection_kind: AttributeSelectionKind = AttributeSelectionKind::Presence;
        let mut kv = KeyValueAttributeSelection {
            name: None,
            selection_kind: AttributeSelectionKind::Presence,
            value: None,
        };

        while let Some(token) = SelectionAttributeToken::next(reader) {
            match token {
                SelectionAttributeToken::Quote(kind) => {
                    if opened_quote.is_none() {
                        opened_quote = Some(kind);
                        position = reader.get_position();
                        continue;
                    }

                    if let Some(quote_kind) = &opened_quote {
                        if *quote_kind != kind {
                            continue;
                        }
                    }

                    opened_quote = None;

                    // `"` and `'` are always of size 1
                    const SIZE_OF_QUOTE: usize = 1;

                    let end_position = reader.get_position() - SIZE_OF_QUOTE;
                    let content_inside_quotes = reader.slice(position..end_position);

                    kv.push(content_inside_quotes);
                }

                SelectionAttributeToken::String(string_value) => {
                    if opened_quote.is_some() {
                        continue;
                    }

                    kv.push(string_value);
                }

                SelectionAttributeToken::StringMatchSelector(equal_selector) => {
                    kv.selection_kind = equal_selector;
                }

                SelectionAttributeToken::Equal => {
                    assert!(kv.name.is_some());
                    assert!(kv.value.is_none());
                    if equal {
                        panic!("Equal should not have been assigned twice");
                    }
                    equal = true;
                }
            }
        }

        if kv.name.is_none() {
            panic!("Need to select a attribute by name");
        }

        kv.refresh_equal();

        AttributeSelection {
            name: kv.name.unwrap(),
            value: kv.value,
            kind: kv.selection_kind,
        }
    }
}

enum SelectionKeyWords<'query> {
    String(&'query str),
    ID,
    CLASS,
    Quote,
    OpenAttribute,  // [
    CloseAttribute, // ]
}

impl<'a> SelectionKeyWords<'a> {
    pub fn next(reader: &mut Reader<'a>) -> Option<Self> {
        let start_pos = reader.get_position();

        if let Some(token) = reader.peek() {
            if matches!(token, b'>' | b' ' | b'+' | b'~' | b'|') {
                return None;
            };
        }

        return match reader.next()? {
            b'#' => Some(Self::ID),
            b'.' => Some(Self::CLASS),
            b'"' => Some(Self::Quote),
            b'\'' => Some(Self::Quote),
            b'[' => Some(Self::OpenAttribute),
            b']' => Some(Self::CloseAttribute),
            _ => {
                // Find end of word
                // POTENTIAL BUG ??? |> I'm pretty sure this is missing '\'' and '"'
                reader.next_until_char_list(&[b' ', b'#', b'.', b'[']);
                return Some(Self::String(reader.slice(start_pos..reader.get_position())));
            }
        };
    }
}

enum SelectionAttributeToken<'a> {
    String(&'a str),
    Quote(QuoteKind),
    Equal,
    StringMatchSelector(AttributeSelectionKind),
}

impl<'a> SelectionAttributeToken<'a> {
    pub fn next(reader: &mut Reader<'a>) -> Option<Self> {
        reader.next_while_char(b' ');

        let start_pos = reader.get_position();

        return match reader.next()? {
            b'"' => Some(Self::Quote(QuoteKind::DoubleQuoted)),
            b'\'' => Some(Self::Quote(QuoteKind::SingleQuoted)),
            b'=' => Some(Self::Equal),
            b'~' => Some(Self::StringMatchSelector(
                AttributeSelectionKind::WhitespaceSeparated,
            )),
            b'|' => Some(Self::StringMatchSelector(
                AttributeSelectionKind::HyphenSeparated,
            )),
            b'^' => Some(Self::StringMatchSelector(AttributeSelectionKind::Prefix)),
            b'$' => Some(Self::StringMatchSelector(AttributeSelectionKind::Suffix)),
            b'*' => Some(Self::StringMatchSelector(AttributeSelectionKind::Substring)),
            b']' => None,
            _ => {
                // Find end of word
                reader.next_until_char_list(&[
                    b' ', b'"', b'\'', b'=', b']', b'~', b'|', b'^', b'$', b'*',
                ]);

                return Some(Self::String(reader.slice(start_pos..reader.get_position())));
            }
        };
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct QueryElement<'a> {
    pub name: Option<&'a str>,
    pub id: Option<&'a str>,
    pub class: Option<&'a str>,
    pub attributes: Vec<AttributeSelection<'a>>,
}

// TODO: I don't like this abstraction
// 1) `build` should be called something like `from`
// 2) The data (Element struct) should be seperated from the parsing logic
// 2.1) The parsing logic should continue to use the iterator parttern I have been using.
// 2.1.1) The flow should look like this => Reader -> Tokenizer -> ElementIterator -> SelectionIterator
impl<'a> QueryElement<'a> {
    pub(crate) fn new(
        name: Option<&'a str>,
        id: Option<&'a str>,
        class: Option<&'a str>,
        attributes: Vec<AttributeSelection<'a>>,
    ) -> Self {
        return Self {
            name: name,
            id: id,
            class: class,
            attributes: attributes,
        };
    }

    fn parse_attribute(&mut self, reader: &mut Reader<'a>) {
        let attribute = AttributeSelection::from(reader);
        self.attributes.push(attribute);
    }
}

impl<'a> From<&mut Reader<'a>> for QueryElement<'a> {
    fn from(reader: &mut Reader<'a>) -> Self {
        let mut element = Self {
            name: None,
            id: None,
            class: None,
            attributes: Vec::new(),
        };

        let mut previous: Option<SelectionKeyWords> = None;

        while let Some(word) = SelectionKeyWords::next(reader) {
            match (previous, &word) {
                (Option::None, SelectionKeyWords::String(name)) => {
                    assert_eq!(element.name, None);
                    element.name = Some(*name);
                }
                (Some(SelectionKeyWords::ID), SelectionKeyWords::String(id_name)) => {
                    if element.id.is_none() {
                        element.id = Some(*id_name);
                    }
                }
                (Some(SelectionKeyWords::CLASS), SelectionKeyWords::String(class_name)) => {
                    // BUG: their is more class that means it should be a list of class
                    element.class = Some(*class_name);
                }
                (_, SelectionKeyWords::OpenAttribute) => element.parse_attribute(reader),

                (Some(SelectionKeyWords::ID), _) | (Some(SelectionKeyWords::CLASS), _) => (),

                (_, _) => (),
            }

            previous = Some(word);
        }

        return element;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection() {
        let mut reader = Reader::new("element#id.class");
        let element = QueryElement::from(&mut reader);

        assert_eq!(
            element,
            QueryElement {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: Vec::new(),
            }
        );
    }

    #[test]
    fn test_fully_detailed_element_selection() {
        let mut reader = Reader::new("element#id.class[selected=true]");

        let element = QueryElement::from(&mut reader);

        assert_eq!(
            element,
            QueryElement {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: Vec::from([AttributeSelection {
                    name: "selected",
                    value: Some("true"),
                    kind: AttributeSelectionKind::Exact
                }]),
            }
        );
    }

    #[test]
    fn test_two_fully_detailed_element_selection() {
        let mut reader = Reader::new("element#id.class[href~=\"_blank\"][selected=true]");

        let element = QueryElement::from(&mut reader);

        assert_eq!(
            element,
            QueryElement {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: Vec::from([
                    AttributeSelection {
                        name: "href",
                        value: Some("_blank"),
                        kind: AttributeSelectionKind::WhitespaceSeparated
                    },
                    AttributeSelection {
                        name: "selected",
                        value: Some("true"),
                        kind: AttributeSelectionKind::Exact
                    }
                ]),
            }
        );
    }

    #[test]
    fn test_handle_duplicates_in_element_definition() {
        let mut reader = Reader::new("element#id.class[selected=true]#id#notid");
        // Since this is used by the developer is acceptable to throw an error in the system
        let element = QueryElement::from(&mut reader);

        assert_eq!(
            element,
            QueryElement {
                name: Some("element"),
                id: Some("id"),
                class: Some("class"),
                attributes: Vec::from([AttributeSelection {
                    name: "selected",
                    value: Some("true"),
                    kind: AttributeSelectionKind::Exact
                }]),
            }
        );
    }
}
