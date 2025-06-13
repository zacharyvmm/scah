use crate::utils::reader::Reader;
use crate::utils::token::QuoteKind;

#[derive(Debug, PartialEq)]
enum AttributeSelectionKind {
    Presence,            // [attribute]
    Exact,               // [attribute=value]
    WhitespaceSeparated, // [attribute~=value]
    HyphenSeparated,     // [attribute|=value]
    Prefix,              // [attribute^=value]
    Suffix,              // [attribute$=value]
    Substring,           // [attribute*=value]
}

enum CombinatorKind {
    Child,             // `>`
    Descendant,        // ` `
    NextSibling,       // `+`
    SubsequentSibling, // `~`

    // I'm pretty sure this does not apply to the scope of the project.
    Namespace, // `|`
}

#[derive(Debug, PartialEq)]
struct AttributeSelection<'a> {
    name: &'a str,
    value: Option<&'a str>,
    kind: AttributeSelectionKind,
}

#[derive(Debug, PartialEq)]
struct Element<'a> {
    name: Option<&'a str>,
    id: Option<&'a str>,
    class: Option<&'a str>,
    attributes: Vec<AttributeSelection<'a>>,
}

enum QueryKind<'a> {
    Element(Element<'a>),

    Combinator(CombinatorKind),

    Has(Element<'a>), // `:has()`
    Not(Element<'a>), // `:not()`

    Or(Box<Self>), // This is the `,` on a selection. ex: `a#hello > p, p.world`
}

struct Selection<'a> {
    query: Vec<QueryKind<'a>>,
}

enum SelectionKeyWords<'a> {
    String(&'a str),
    ID,
    CLASS,
    Quote,
    OpenAttribute,  // [
    CloseAttribute, // ]
}

impl<'a> SelectionKeyWords<'a> {
    pub fn next(reader: &mut Reader<'a>) -> Option<Self> {
        let start_pos = reader.get_position();

        return match reader.next()? {
            '#' => Some(Self::ID),
            '.' => Some(Self::CLASS),
            '"' => Some(Self::Quote),
            '\'' => Some(Self::Quote),
            '[' => Some(Self::OpenAttribute),
            ']' => Some(Self::CloseAttribute),
            '>' | ' ' | '+' | '~' | '|' => None,
            _ => {
                // Find end of word
                reader.next_while(|c| !c.is_whitespace() && c != '#' && c != '.' && c != '[');
                return Some(Self::String(reader.slice(start_pos..reader.get_position())));
            }
        };
    }
}

enum SelectionAttributeToken<'a> {
    String(&'a str),
    Quote(QuoteKind),
    Equal,
    EqualSelector(AttributeSelectionKind),
}

impl<'a> SelectionAttributeToken<'a> {
    pub fn next(reader: &mut Reader<'a>) -> Option<Self> {
        reader.next_while(|c| c.is_whitespace());

        let start_pos = reader.get_position();

        return match reader.next()? {
            '"' => Some(Self::Quote(QuoteKind::DoubleQuoted)),
            '\'' => Some(Self::Quote(QuoteKind::SingleQuoted)),
            '=' => Some(Self::Equal),
            '~' => Some(Self::EqualSelector(
                AttributeSelectionKind::WhitespaceSeparated,
            )),
            '|' => Some(Self::EqualSelector(AttributeSelectionKind::HyphenSeparated)),
            '^' => Some(Self::EqualSelector(AttributeSelectionKind::Prefix)),
            '$' => Some(Self::EqualSelector(AttributeSelectionKind::Suffix)),
            '*' => Some(Self::EqualSelector(AttributeSelectionKind::Substring)),
            ']' => None,
            _ => {
                // Find end of word
                reader.next_while(|c| {
                    !c.is_whitespace()
                        && c != '"'
                        && c != '\''
                        && c != '='
                        && c != ']'
                        && c != '~'
                        && c != '|'
                        && c != '^'
                        && c != '$'
                        && c != '*'
                });
                return Some(Self::String(reader.slice(start_pos..reader.get_position())));
            }
        };
    }
}

impl<'a> Element<'a> {
    fn new() -> Self {
        return Self {
            name: None,
            id: None,
            class: None,
            attributes: Vec::new(),
        };
    }

    fn handle_attribute_parsing(&mut self, reader: &mut Reader<'a>) {
        let mut opened_quote: Option<QuoteKind> = None;
        let mut position = reader.get_position();
        let mut equal = false;

        let mut name: Option<&str> = None;
        let mut value: Option<&str> = None;
        let mut kind: Option<AttributeSelectionKind> = None;

        while let Some(token) = SelectionAttributeToken::next(reader) {
            match (&opened_quote, token) {
                (Option::None, SelectionAttributeToken::Quote(kind)) => {
                    opened_quote = Some(kind);
                    position = reader.get_position();
                }
                (Some(previous_kind), SelectionAttributeToken::Quote(kind)) => {
                    if *previous_kind != kind {
                        continue;
                    }

                    opened_quote = None;

                    // `"` and `'` are always of size 1
                    const SIZE_OF_QUOTE: usize = 1;

                    let end_position = reader.get_position() - SIZE_OF_QUOTE;
                    let content_inside_quotes = reader.slice(position..end_position);

                    if !name.is_some() {
                        name = Some(content_inside_quotes);
                    } else if !value.is_some() {
                        value = Some(content_inside_quotes);
                    } else {
                        panic!("This is not supposed to happen");
                    }
                }

                (Option::None, SelectionAttributeToken::String(string_value)) => {
                    if !name.is_some() {
                        name = Some(string_value);
                    } else if !value.is_some() {
                        value = Some(string_value);
                    } else {
                        panic!("This is not supposed to happen");
                    }
                }

                (_, SelectionAttributeToken::EqualSelector(equal_selector)) => {
                    kind = Some(equal_selector);
                }

                (_, SelectionAttributeToken::Equal) => {
                    if equal {
                        panic!("Equal should not have been assigned twice");
                    }
                    equal = true;
                }

                (_, _) => (),
            }
        }

        if !name.is_some() {
            panic!("Need to select a attribute by name");
        }

        if !kind.is_some() {
            if value.is_some() {
                kind = Some(AttributeSelectionKind::Exact);
            } else {
                kind = Some(AttributeSelectionKind::Presence);
            }
        }

        self.attributes.push(AttributeSelection {
            name: name.unwrap(),
            value: value,
            kind: kind.unwrap(),
        });
    }

    fn build(&mut self, reader: &mut Reader<'a>) {
        let mut previous: Option<SelectionKeyWords> = None;

        while let Some(word) = SelectionKeyWords::next(reader) {
            match (previous, &word) {
                (Option::None, SelectionKeyWords::String(name)) => {
                    self.name = Some(name);
                }
                (Some(SelectionKeyWords::ID), SelectionKeyWords::String(id_name)) => {
                    self.id = Some(id_name);
                }
                (Some(SelectionKeyWords::CLASS), SelectionKeyWords::String(class_name)) => {
                    self.class = Some(class_name);
                }
                (_, SelectionKeyWords::OpenAttribute) => self.handle_attribute_parsing(reader),

                (Some(SelectionKeyWords::ID), _) | (Some(SelectionKeyWords::CLASS), _) => (),

                (_, _) => (),
            }

            previous = Some(word);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_element_selection() {
        let mut reader = Reader::new("element#id.class");
        let mut element = Element::new();
        element.build(&mut reader);

        assert_eq!(
            element,
            Element {
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

        let mut element = Element::new();
        element.build(&mut reader);

        assert_eq!(
            element,
            Element {
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

        let mut element = Element::new();
        element.build(&mut reader);

        assert_eq!(
            element,
            Element {
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
        let reader = Reader::new("element#id.class[selected=true]#id#notid");
        // Since this is used by the developer is acceptable to throw an error in the system
        assert!(false);
    }
}
