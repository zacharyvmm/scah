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
        reader.next_while(|c| c != '<');

        if reader.peek().is_none() {
            return false;
        }

        self.content.push(reader, reader.get_position());
        //self.content.set_start(reader.get_position());
        let tag = XHtmlTag::from(&mut *reader);

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
                while let Some(item) = &mut self.stack.pop() {
                    if let Some(body) = &mut item.body {
                        if let Some(range) = &mut body.content.inner_html {
                            //inner_html = &inner_html[..reader.get_position()]
                            range.end = reader.get_position();
                        }

                        if let Some(range) = &mut body.content.text_content {
                            range.end = self.content.get_position() - 1;

                            //text_content.end =
                        }
                    }

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
    use crate::css::selectors::{ElementContent, SelectorQuery, SelectorQueryKind, Selectors};

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
            data: ElementContent {
                inner_html: false,
                text_content: true,
                //attributes: Vec::new(),
            },
        }]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        //while parser.next(&mut reader) {}
    }

    #[test]
    fn test_text_content_and_inner_html() {
        let mut reader = Reader::new(basic_html);
        let queries = Vec::from([]);

        let mut parser = XHtmlParser::new(Selectors::new(queries));

        let mut continue_parser = parser.next(&mut reader); // <html>
        assert_eq!(true, continue_parser);

        continue_parser = parser.next(&mut reader); // <h1>
        assert_eq!(true, continue_parser);

        continue_parser = parser.next(&mut reader); // </h1>
        assert_eq!(true, continue_parser);
        assert_eq!(parser.content.list, Vec::from(["Hello World"]));

        continue_parser = parser.next(&mut reader); // <p class="indent">
        assert_eq!(true, continue_parser);
        assert_eq!(parser.content.list, Vec::from(["Hello World"]));

        continue_parser = parser.next(&mut reader); // <span id="name" class="bold">
        assert_eq!(true, continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is"])
        );

        continue_parser = parser.next(&mut reader); // </span>
        assert_eq!(true, continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );

        continue_parser = parser.next(&mut reader); // </p>
        assert_eq!(true, continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );

        continue_parser = parser.next(&mut reader); // </html>
        assert_eq!(true, continue_parser);
        assert_eq!(
            parser.content.list,
            Vec::from(["Hello World", "My name is", "Zachary"])
        );
    }
}
