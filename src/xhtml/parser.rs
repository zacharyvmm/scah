use super::element::element::{XHtmlElement, XHtmlTag};
use crate::utils::reader::Reader;
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct BodyContent<'a> {
    element: XHtmlElement<'a>,
    text_content: Option<Range<usize>>,
    inner_html: Option<&'a str>,
}

#[derive(Debug, PartialEq)]
struct StackItem<'a> {
    name: &'a str,
    body: Option<BodyContent<'a>>,
}

struct XHtmlParser<'a> {
    stack: Vec<StackItem<'a>>,
}

impl<'a> XHtmlParser<'a> {
    pub fn new() -> Self {
        return Self { stack: Vec::new() };
    }

    pub fn next(&mut self, reader: &mut Reader<'a>) -> Option<XHtmlElement<'a>> {
        // move until it finds the first `<`
        reader.next_upto(|c| c != '<');

        if reader.peek().is_none() {
            return None;
        }

        // TODO: I need to handle the start and end position

        let tag = XHtmlTag::from(&mut *reader);

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());

        match tag {
            XHtmlTag::Open(element) => {
                // TODO: if conforms a FSM end then add the optional body

                // Check already created FSM's list
                // Check new FSM list

                self.stack.push(StackItem {
                    name: element.name,
                    body: None,
                });

                return Some(element);
            }
            XHtmlTag::Close(closing_tag) => {
                while let Some(item) = self.stack.pop() {
                    if item.name == closing_tag {
                        break;
                    }
                }
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(
            r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#,
        );

        let mut parser = XHtmlParser::new();

        // STEP 1
        let mut element = parser.next(&mut reader);
        assert_eq!(
            element,
            Some(XHtmlElement {
                name: "html",
                id: None,
                class: None,
                attributes: Vec::new()
            })
        );
        assert_eq!(
            parser.stack,
            Vec::from([StackItem {
                name: "html",
                body: None
            }])
        );

        // STEP 2
        element = parser.next(&mut reader);
        assert_eq!(
            element,
            Some(XHtmlElement {
                name: "h1",
                id: None,
                class: None,
                attributes: Vec::new()
            })
        );
        assert_eq!(
            parser.stack,
            Vec::from([
                StackItem {
                    name: "html",
                    body: None
                },
                StackItem {
                    name: "h1",
                    body: None
                }
            ])
        );

        // STEP 3
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(
            parser.stack,
            Vec::from([StackItem {
                name: "html",
                body: None
            }])
        );

        // STEP 4
        element = parser.next(&mut reader);
        assert_eq!(
            element,
            Some(XHtmlElement {
                name: "p",
                id: None,
                class: Some("indent"),
                attributes: Vec::new()
            })
        );
        assert_eq!(
            parser.stack,
            Vec::from([
                StackItem {
                    name: "html",
                    body: None
                },
                StackItem {
                    name: "p",
                    body: None
                }
            ])
        );

        // STEP 5
        element = parser.next(&mut reader);
        assert_eq!(
            element,
            Some(XHtmlElement {
                name: "span",
                id: Some("name"),
                class: Some("bold"),
                attributes: Vec::new()
            })
        );
        assert_eq!(
            parser.stack,
            Vec::from([
                StackItem {
                    name: "html",
                    body: None
                },
                StackItem {
                    name: "p",
                    body: None
                },
                StackItem {
                    name: "span",
                    body: None
                }
            ])
        );

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(
            parser.stack,
            Vec::from([
                StackItem {
                    name: "html",
                    body: None
                },
                StackItem {
                    name: "p",
                    body: None
                }
            ])
        );

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(
            parser.stack,
            Vec::from([StackItem {
                name: "html",
                body: None
            }])
        );

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack.len(), 0);
    }
}
