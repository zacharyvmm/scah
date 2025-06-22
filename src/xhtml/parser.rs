use super::element::parser::{XHtmlElement, XHtmlTag};
use crate::utils::reader::Reader;

struct XHtmlParser<'a> {
    stack: Vec<&'a str>,
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

        reader.next_while(|c| c.is_whitespace());

        match tag {
            XHtmlTag::Open(element) => {
                self.stack.push(element.name);

                return Some(element);
            }
            XHtmlTag::Close(closing_tag) => {
                while let Some(name) = self.stack.pop() {
                    if name == closing_tag {
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
        assert_eq!(parser.stack, Vec::from(["html"]));

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
        assert_eq!(parser.stack, Vec::from(["html", "h1"]));

        // STEP 3
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack, Vec::from(["html"]));

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
        assert_eq!(parser.stack, Vec::from(["html", "p"]));

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
        assert_eq!(parser.stack, Vec::from(["html", "p", "span"]));

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack, Vec::from(["html", "p"]));

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack, Vec::from(["html"]));

        // STEP 6
        element = parser.next(&mut reader);
        assert_eq!(element, None);
        assert_eq!(parser.stack.len(), 0);
    }
}
