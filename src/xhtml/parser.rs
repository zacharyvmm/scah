use super::element::element::XHtmlTag;
use super::text_content::TextContent;
use crate::css::DocumentPosition;
use crate::css::FsmManager;
use crate::store::Store;
use crate::utils::Reader;

pub struct XHtmlParser<'html, 'query, S>
where
    S: Store<'html, 'query>,
{
    position: DocumentPosition,
    pub content: TextContent<'html>,
    pub selectors: FsmManager<'html, 'query, S>,
}

impl<'html, 'query, S> XHtmlParser<'html, 'query, S>
where
    S: Store<'html, 'query>,
{
    pub fn new(selectors: FsmManager<'query, 'html, S>) -> Self {
        return Self {
            position: DocumentPosition {
                element_depth: 0,
                reader_position: 0, // for inner_html
                text_content_position: 0,
            },
            content: TextContent::new(),
            selectors: selectors,
        };
    }

    pub fn next(&mut self, reader: &mut Reader<'html>) -> bool {
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
                self.position.reader_position = reader.get_position();
                tag = XHtmlTag::from(&mut *reader);
                self.content.set_start(reader.get_position());
            }

            tag.unwrap()
        };

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());

        match tag {
            XHtmlTag::Open(element) => {
                println!("opened: `{}`", element.name);
                self.position.element_depth += 1;
                self.position.reader_position = reader.get_position();
                self.selectors.next(element, &self.position);
            }
            XHtmlTag::Close(closing_tag) => {
                println!("closed: `{closing_tag}`");
                self.position.element_depth -= 1;
                self.selectors.back(closing_tag, &self.position);
            }
        }

        !reader.eof()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::{FsmManager, Save, Selection, SelectionKind, SelectionPart};
    use crate::store::{Element, RustStore};
    use crate::utils::Reader;
    use crate::xhtml::element::element::{Attribute, XHtmlElement};

    const BASIC_HTML: &str = r#"
        <html>
            <h1>Hello World</h1>
            <p class="indent">
                My name is <span id="name" class="bold">Zachary</span>
            </p>
        </html>
        "#;

    #[test]
    fn test_basic_html() {
        let mut reader = Reader::new(BASIC_HTML);

        let section = SelectionPart::new(
            "p.indent > .bold",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = Selection::new(section);

        let queries = vec![selection_tree];

        let manager = FsmManager::<RustStore>::new(&queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            println!("{:?}", parser.selectors);
        }

        // assert_eq!(
        //     parser.selectors.matches()[0].list[1].value,
        //     XHtmlElement {
        //         name: "span",
        //         id: Some("name"),
        //         class: Some("bold"),
        //         attributes: vec![]
        //     }
        // )
    }

    #[test]
    fn test_text_content() {
        let mut reader = Reader::new(BASIC_HTML);

        let section = SelectionPart::new(
            "p.indent > .bold",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        );
        let selection_tree = Selection::new(section);

        let queries = vec![selection_tree];
        let manager = FsmManager::<RustStore>::new(&queries);

        let mut parser = XHtmlParser::new(manager);

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
    fn test_top_level_multi_selection() {
        let mut reader = Reader::new(BASIC_HTML);

        let selection_tree_1 = Selection::new(SelectionPart::new(
            "p.indent > .bold",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        ));

        let selection_tree_2 = Selection::new(SelectionPart::new(
            "h1 + .indent #name",
            SelectionKind::All(Save {
                inner_html: false,
                text_content: false,
            }),
        ));

        let queries = vec![selection_tree_1, selection_tree_2];
        let manager = FsmManager::<RustStore>::new(&queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            println!("{:?}", parser.selectors);
        }

        // let matches = parser.selectors.matches();
        // assert_eq!(
        //     matches[0].list[1].value,
        //     XHtmlElement {
        //         name: "span",
        //         id: Some("name"),
        //         class: Some("bold"),
        //         attributes: vec![]
        //     }
        // );
        // assert_eq!(
        //     matches[1].list[1].value,
        //     XHtmlElement {
        //         name: "span",
        //         id: Some("name"),
        //         class: Some("bold"),
        //         attributes: vec![]
        //     }
        // );
    }
}
