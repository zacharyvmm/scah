#[derive(Debug, PartialEq, Clone)]
pub struct Attribute<'html> {
    pub key: &'html [u8],
    pub value: Option<&'html [u8]>,
}

pub type Attributes<'html> = Vec<Attribute<'html>>;
//pub type Attributes<'html> = SmallVec<[Attribute<'html>, 3]>;

#[derive(Debug, PartialEq, Clone)]
pub struct XHtmlElement<'html> {
    pub closing: bool,
    pub name: &'html [u8],
    pub id: Option<&'html [u8]>,
    pub class: Option<&'html [u8]>,
    pub attributes: Attributes<'html>,
}

impl<'html> XHtmlElement<'html> {
    fn new() -> Self {
        Self {
            closing: false,
            name: &[],
            id: None,
            class: None,
            attributes: vec![],
        }
    }

    fn clear(&mut self) {
        self.name = &[];
        self.id = None;
        self.class = None;
        self.attributes.clear();
        self.closing = false;
    }
}

#[derive(Debug, PartialEq, Clone)]
enum QuoteKind {
    Double,
    Single,
}

#[derive(Debug, PartialEq, Clone)]
enum ElementFSM {
    None,
    Element,
    Closing,
    Quote(QuoteKind),
    Assign,
}

impl Default for ElementFSM {
    fn default() -> Self {
        Self::None
    }
}

// only check for `<`, `>`, ` `, `"`, `'`, `=`, `/`

impl ElementFSM {
    fn step(&mut self, character: u8) {
        *self = match (&self, character) {
            (Self::None, b'>') => unreachable!(),
            (Self::None, b'<') => Self::Element,
            (Self::None, _) => Self::None,

            (Self::Element, b'<') => unreachable!(),
            (Self::Element, b'>') => Self::None,
            (Self::Element, b' ') => Self::Element,
            (Self::Element, b'"') => Self::Quote(QuoteKind::Double),
            (Self::Element, b'\'') => Self::Quote(QuoteKind::Single),
            (Self::Element, b'=') => Self::Assign,
            (Self::Element, b'/') => Self::Closing,

            (Self::Quote(QuoteKind::Double), b'"') => Self::Element,
            (Self::Quote(QuoteKind::Double), _) => Self::Quote(QuoteKind::Double),

            (Self::Quote(QuoteKind::Single), b'\'') => Self::Element,
            (Self::Quote(QuoteKind::Single), _) => Self::Quote(QuoteKind::Single),

            (Self::Assign, b'<') => unreachable!(),
            (Self::Assign, b'=') => unreachable!(),
            (Self::Assign, b' ') => Self::Assign,
            (Self::Assign, b'"') => Self::Quote(QuoteKind::Double),
            (Self::Assign, b'\'') => Self::Quote(QuoteKind::Single),

            _ => Self::None,
        };
    }
}

// NOTE: This element only exist while
pub struct Element<'a> {
    element: XHtmlElement<'a>,
    fsm: ElementFSM,
    index: usize,
}

impl<'a> Element<'a> {
    pub fn new() -> Self {
        Self {
            element: XHtmlElement::new(),
            fsm: ElementFSM::default(),
            index: 0,
        }
    }

    fn add_key_with_no_value(&mut self, key: &'a [u8]) {
        debug_assert!(!key.is_empty());

        if self.element.name.is_empty() {
            self.element.name = key;
        } else {
            self.element.attributes.push(Attribute { key, value: None });
        }
    }

    pub fn next(&mut self, string: &'a [u8], indices: &[u32]) {
        self.element.clear();

        let mut label: &'a [u8] = &[];
        let mut start_position: usize = {
            if self.index == 0 {
                usize::MIN
            } else {
                indices[self.index - 1] as usize + 1
            }
        };
        for i in self.index..indices.len() {
            let idx = indices[i] as usize;
            let character = &string[idx];

            let last_state = self.fsm.clone();

            println!("Character: {} ({})", *character as char, i);
            self.fsm.step(*character);
            println!("States: {:?} and {:?}", last_state, self.fsm);

            type FSM = ElementFSM;
            match (last_state, &self.fsm) {
                // ELEMENT OPEN EVENT
                (FSM::None, FSM::Element) => {
                    // TODO: set inner_html OPEN marker
                    // TODO: set text_content CLOSE marker
                }

                // ELEMENT CLOSE EVENT
                (FSM::Element, FSM::None) => {
                    // TODO: set inner_html CLOSE marker
                    // TODO: set text_content OPEN marker
                    if !label.is_empty() {
                        self.add_key_with_no_value(label);
                    } else if idx > start_position {
                        label = &string[start_position..idx];
                        self.add_key_with_no_value(label);
                    }

                    self.index = i + 1;
                    return;
                }

                (FSM::Element, FSM::Element | FSM::Assign) => {
                    if !label.is_empty() {
                        self.add_key_with_no_value(label);
                    }
                    label = &string[start_position..idx];
                }

                (_, FSM::Closing) => {
                    self.element.closing = true;

                    // Resetting to Element, since Closing is just a flag
                    self.fsm = FSM::Element;
                }

                // OPEN QUOTE EVENT: After assign
                (FSM::Assign, FSM::Quote(_)) => {
                    debug_assert!(!label.is_empty());
                }

                // OPEN QUOTE EVENT
                (FSM::Element, FSM::Quote(_)) => {
                    if !label.is_empty() {
                        self.add_key_with_no_value(label);
                        label = &[];
                    }
                }

                (FSM::Quote(_), FSM::Assign) => {
                    debug_assert_ne!(start_position, usize::MAX);
                    debug_assert!(label.is_empty());
                    let word_range = (start_position)..(idx);
                    let word = &string[word_range];
                    label = word;
                }

                // CLOSE QUOTE EVENT
                (FSM::Quote(_), FSM::Element) => {
                    debug_assert_ne!(start_position, usize::MAX);
                    let word_range = (start_position)..(idx);
                    let word = &string[word_range];

                    if label.is_empty() {
                        label = word;
                    } else {
                        match label {
                            b"id" => self.element.id = Some(word),
                            b"class" => self.element.class = Some(word),
                            &[] => label = word,
                            l => self.element.attributes.push(Attribute {
                                key: l,
                                value: Some(word),
                            }),
                        }

                        label = &[];
                    }
                }

                _ => {}
            }

            start_position = idx + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_fsm() {
        let string = "<div class=\"hello-world\"><a href='my-link'></a></div>";
        let b = string.as_bytes();
        let mut fsm = ElementFSM::default();

        let indices = &[
            0, 4, 10, 11, 23, 24, 25, 27, 32, 33, 41, 42, 43, 44, 46, 47, 48, 52,
        ];
        let expected_states = &[ElementFSM::Element];
    }

    #[test]
    fn test_element() {
        let string = "<div class=\"hello-world\"><a href='my-link'></a></div>";
        let b = string.as_bytes();
        let mut factory = Element::new();

        let indices = &[
            0, 4, 10, 11, 23, 24, 25, 27, 32, 33, 41, 42, 43, 44, 46, 47, 48, 52,
        ];

        // for i in indices {
        //     let index = *i as usize;
        //     print!("{}, ", b[index] as char);
        // }
        factory.next(b, indices);
        assert_eq!(
            factory.element,
            XHtmlElement {
                closing: false,
                name: b"div",
                id: None,
                class: Some(b"hello-world"),
                attributes: vec![],
            }
        );

        factory.next(b, indices);
        assert_eq!(
            factory.element,
            XHtmlElement {
                closing: false,
                name: b"a",
                id: None,
                class: None,
                attributes: vec![Attribute {
                    key: b"href",
                    value: Some(b"my-link")
                }],
            }
        );

        factory.next(b, indices);
        assert_eq!(
            factory.element,
            XHtmlElement {
                closing: true,
                name: b"a",
                id: None,
                class: None,
                attributes: vec![],
            }
        );

        factory.next(b, indices);
        assert_eq!(
            factory.element,
            XHtmlElement {
                closing: true,
                name: b"div",
                id: None,
                class: None,
                attributes: vec![],
            }
        );
    }
}
