use super::element::element::{XHtmlElement, XHtmlTag};
use super::text_content::TextContent;
use crate::css::selectors::Selectors;
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

struct XHtmlParser<'a, 'query> {
    stack: Vec<StackItem<'a>>,
    content: TextContent<'a>,
    selectors: Selectors<'query, 'a>,
}

impl<'a, 'query> XHtmlParser<'a, 'query> {
    pub fn new(selectors: Selectors<'query, 'a>) -> Self {
        return Self {
            stack: Vec::new(),
            content: TextContent::new(),
            selectors: selectors,
        };
    }

    fn depth(&self) -> u8 {
        self.stack.len() as u8
    }

    pub fn next(&mut self, reader: &mut Reader<'a>) -> bool {
        // move until it finds the first `<`
        reader.next_upto(|c| c != '<');

        if reader.peek().is_none() {
            return false;
        }

        // TODO: I need to handle the start and end position

        self.content.push(reader);
        self.content.reset_start();
        let tag = XHtmlTag::from(&mut *reader);

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());

        match tag {
            XHtmlTag::Open(element) => {
                // TODO: if conforms a FSM end then add the optional body

                // Check already created FSM's list
                // Check new FSM list

                self.selectors.feed(&element, self.depth());

                self.content.push(reader);
                self.stack.push(StackItem {
                    name: element.name,
                    body: None,
                });

                return true;
            }
            XHtmlTag::Close(closing_tag) => {
                while let Some(item) = self.stack.pop() {
                    if let Some(body) = item.body {
                        if let Some(inner_html) = body.inner_html {
                            //inner_html = &inner_html[..reader.get_position()]
                        }

                        if let Some(text_content) = body.text_content {
                            self.content.push(reader);

                            //text_content.end =
                        }
                    }

                    if item.name == closing_tag {
                        break;
                    }
                }
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::selectors::{ElementContent, SelectorQuery, SelectorQueryKind, Selectors};

    const basic_html: &str = r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#;

    /*
    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(basic_html);

        let mut parser = XHtmlParser::new(Selectors::new(Vec::new()));

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

        // STEP 7
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(
            parser.stack,
            Vec::from([StackItem {
                name: "html",
                body: None
            }])
        );

        // STEP 8
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack.len(), 0);
    }
    */

    #[test]
    fn test_basic_html_with_selection() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([SelectorQuery {
            kind: SelectorQueryKind::All,
            query: "p.indent > .bold",
            data: ElementContent {
                inner_html: false,
                text_content: true,
                attributes: Vec::new(),
            },
        }]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        while parser.next(&mut reader) {}
    }
}
