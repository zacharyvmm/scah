use super::element::element::XHtmlTag;
use super::text_content::TextContent;
use crate::css::selectors::{Body, Selectors};
use crate::utils::reader::Reader;
use std::ops::Range;

#[derive(Debug, PartialEq)]
struct StackItem<'a> {
    name: &'a str,
    body: Option<Body<'a>>,
}

pub struct XHtmlParser<'a, 'query> {
    stack: Vec<StackItem<'a>>,
    pub content: TextContent<'a>,
    pub selectors: Selectors<'query, 'a>,
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
        reader.next_while(|c| c != '<');

        if reader.peek().is_none() {
            return false;
        }
        let before_element_position = reader.get_position();

        self.content.push(reader, before_element_position);
        //self.content.set_start(reader.get_position());
        let tag = {
            let mut tag: Option<XHtmlTag> = None;

            while tag.is_none() {
                tag = XHtmlTag::from(&mut *reader);
            }

            tag.unwrap()
        };

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());

        match tag {
            XHtmlTag::Open(element) => {
                self.content.set_start(reader.get_position());

                // TODO: if conforms a FSM end then add the optional body

                // Check already created FSM's list
                // Check new FSM list

                let name = element.name;

                let need_more_info = self.selectors.feed(element, self.depth());

                if need_more_info.is_none() {
                    self.stack.push(StackItem {
                        name: name,
                        body: None,
                    });

                    return true;
                }

                let (wait, mut body) = need_more_info.unwrap();
                assert!(wait.text_content | wait.inner_html);

                if wait.text_content {
                    body.content.text_content = Some(Range {
                        start: self.content.get_position(),
                        end: self.content.get_position(),
                    })
                }

                if wait.inner_html {
                    body.content.inner_html = Some(Range {
                        start: reader.get_position(),
                        end: reader.get_position(),
                    })
                }
                self.stack.push(StackItem {
                    name: name,
                    body: Some(body),
                });

                return true;
            }
            XHtmlTag::Close(closing_tag) => {
                self.content.set_start(reader.get_position());
                while let Some(item) = self.stack.pop() {
                    if let Some(mut body) = item.body {
                        if let Some(ref mut range) = body.content.inner_html {
                            range.end = before_element_position;
                        }

                        if let Some(ref mut range) = body.content.text_content {
                            range.end = self.content.get_position();
                        }

                        self.selectors.on_stack_pop(body);
                    }

                    self.selectors.back(self.depth());

                    if item.name == closing_tag {
                        break;
                    }
                }
                return self.stack.len() != 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::selection_map::{BodyContent, Select, SelectionMap};
    use crate::css::selectors::{InnerContent, SelectorQuery, SelectorQueryKind, Selectors};
    use crate::xhtml::element::element::XHtmlElement;

    const basic_html: &str = r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#;

    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(basic_html);

        let mut parser = XHtmlParser::new(Selectors::new(Vec::new()));

        // STEP 1
        let mut continue_parser = parser.next(&mut reader);
        assert!(continue_parser);
        assert_eq!(
            parser.stack,
            Vec::from([StackItem {
                name: "html",
                body: None
            }])
        );

        // STEP 2
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);
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
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);

        // STEP 4
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);
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
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);
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
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);
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
        continue_parser = parser.next(&mut reader);
        assert!(continue_parser);

        // STEP 8
        continue_parser = parser.next(&mut reader);
        assert!(!continue_parser);
        assert_eq!(parser.stack.len(), 0);
    }

    #[test]
    fn test_basic_html_with_selection() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([SelectorQuery {
            kind: SelectorQueryKind::All,
            query: "p.indent > .bold",
            data: InnerContent {
                inner_html: false,
                text_content: false,
                //attributes: Vec::new(),
            },
        }]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        while parser.next(&mut reader) {}

        assert_eq!(
            parser.selectors.map,
            SelectionMap {
                elements: Vec::from([BodyContent {
                    element: XHtmlElement {
                        name: "span",
                        id: Some("name"),
                        class: Some("bold"),
                        attributes: Vec::new()
                    },
                    text_content: None,
                    inner_html: None
                }]),
                mappings: Vec::from([("p.indent > .bold", Select::All(Vec::from([0])))]),
            }
        );
    }

    #[test]
    fn test_basic_html_with_selection_with_text_content_and_inner_html() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([SelectorQuery {
            kind: SelectorQueryKind::First,
            query: "p.indent > .bold",
            data: InnerContent {
                inner_html: true,
                text_content: true,
                //attributes: Vec::new(),
            },
        }]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        while parser.next(&mut reader) {}

        assert_eq!(
            parser.content.join(
                parser.selectors.map.elements[0]
                    .text_content
                    .clone()
                    .unwrap()
            ),
            "Zachary"
        );
        assert_eq!(
            reader.slice(parser.selectors.map.elements[0].inner_html.clone().unwrap()),
            "Zachary"
        );

        assert_eq!(
            parser.selectors.map.mappings,
            Vec::from([("p.indent > .bold", Select::One(Some(0)))]),
        );
    }

    #[test]
    fn test_text_content() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        let mut continue_parser = parser.next(&mut reader); // <html>
        assert!(continue_parser);

        continue_parser = parser.next(&mut reader); // <h1>
        assert!(continue_parser);

        continue_parser = parser.next(&mut reader); // </h1>
        assert!(continue_parser);
        assert_eq!(parser.content.list, Vec::from(["Hello World"]));

        continue_parser = parser.next(&mut reader); // <p class="indent">
        assert!(continue_parser);
        assert_eq!(parser.content.list, Vec::from(["Hello World"]));

        continue_parser = parser.next(&mut reader); // <span id="name" class="bold">
        assert!(continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is"])
        );

        continue_parser = parser.next(&mut reader); // </span>
        assert!(continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );

        continue_parser = parser.next(&mut reader); // </p>
        assert!(continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );

        continue_parser = parser.next(&mut reader); // </html>
        assert!(!continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );
    }

    #[test]
    fn test_fsm_states() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([SelectorQuery {
            kind: SelectorQueryKind::All,
            query: "p.indent > .bold",
            data: InnerContent {
                inner_html: false,
                text_content: false,
                //attributes: Vec::new(),
            },
        }]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        while parser.next(&mut reader) {}
    }
}
