use super::element::element::XHtmlTag;
use super::text_content::TextContent;
use crate::css::DocumentPosition;
use crate::css::FsmManager;
use crate::dbg_print;
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
        reader.next_while(|c| c != b'<');

        if reader.peek().is_none() {
            return false;
        }

        let tag = {
            let mut tag: Option<XHtmlTag> = None;

            while tag.is_none() {
                self.position.reader_position = reader.get_position();
                tag = XHtmlTag::from(&mut *reader);
                if tag.is_none() && self.content.text_start.is_some() {
                    if let Some(position) = self.content.push(reader, self.position.reader_position)
                    {
                        self.position.text_content_position = position;
                        self.content.set_start(reader.get_position());
                    }
                }
            }

            tag.unwrap()
        };

        if self.content.text_start.is_some() {
            if let Some(position) = self.content.push(reader, self.position.reader_position) {
                self.position.text_content_position = position;
            }
        }

        self.content.set_start(reader.get_position());

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());
        let mut early_exit = false;

        match tag {
            XHtmlTag::Open(element) => {
                self.position.element_depth += 1;
                self.position.reader_position = reader.get_position();

                dbg_print!(
                    "opening: `{}` ({})",
                    element.name,
                    self.position.element_depth
                );

                let mut remove_depth_after_next = false;
                if element.is_self_closing() {
                    remove_depth_after_next = true;
                }

                self.selectors.next(element, &self.position);

                if remove_depth_after_next {
                    self.position.element_depth -= 1;
                }
            }
            XHtmlTag::Close(closing_tag) => {
                dbg_print!("closing: `{closing_tag}` ({})", self.position.element_depth);

                early_exit =
                    self.selectors
                        .back(closing_tag, &self.position, reader, &self.content);
                self.position.element_depth -= 1;
            }
        }

        !early_exit && !reader.eof()
    }

    pub fn matches(self) -> S {
        self.selectors.matches()
    }
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::css::{FsmManager, Query, Save, SelectionKind, SelectionPart};
    use crate::store::{Element, RustStore, SelectionValue, Store, ValueKind};
    use crate::utils::Reader;
    use crate::xhtml::element::element::{Attribute, XHtmlElement};
    use pretty_assertions::{assert_eq, assert_ne};

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

        let queries = &[Query::all("p.indent > .bold", Save::none()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let root = *parser.selectors.matches().root;

        assert_eq!(
            root,
            Element {
                name: "root",
                id: None,
                class: None,
                attributes: vec![],
                inner_html: None,
                text_content: None,
                children: HashMap::from([(
                    "p.indent > .bold",
                    SelectionValue {
                        kind: ValueKind::List,
                        list: vec![Element {
                            name: "span",
                            id: Some("name"),
                            class: Some("bold"),
                            attributes: vec![],
                            inner_html: None,
                            text_content: None,
                            children: HashMap::new(),
                        }]
                    }
                )])
            }
        )
    }

    #[test]
    fn test_text_content() {
        let mut reader = Reader::new(BASIC_HTML);

        let queries = &[Query::all("p.indent > .bold", Save::none()).build()];
        let manager = FsmManager::new(RustStore::new(false), queries);

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

        let queries = &[
            Query::all("p.indent > .bold", Save::none()).build(),
            Query::all("h1 + .indent #name", Save::none()).build(),
        ];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("Queries: {:#?}", queries);

        while parser.next(&mut reader) {}
        println!("Selectors: {:#?}", parser.selectors);
    }

    const MORE_ADVANCED_BASIC_HTML: &str = r#"
        <html>
            <h1>Hello World</h1>
            <main>
                <section>
                    <a href="https://hello.com">Hello</a>
                    <div>
                        <a href="https://world.com">World</a>
                    </div>
                </section>
            </main>

            <main>
                <section>
                    <a href="https://hello2.com">Hello2</a>

                    <div>
                        <a href="https://world2.com">World2</a>
                        <div>
                            <a href="https://world3.com">World3</a>
                        </div>
                    </div>
                </section>
            </main>
        </html>
        "#;

    #[test]
    fn test_multi_selection() {
        let mut reader = Reader::new(MORE_ADVANCED_BASIC_HTML);
        let queries = Query::all("main > section", Save::all()).then(|section| {
            [
                section.first("> a[href]", Save::all()),
                section.all("div a", Save::all()),
            ]
        });
        let queries = &[queries.build()];
        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        while parser.next(&mut reader) {}

        let map = parser.matches().root.children;
        //println!("Map: {:#?}", map);
        assert_eq!(
            map,
            HashMap::from([(
                "main > section",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![
                        Element {
                            name: "section",
                            id: None,
                            class: None,
                            attributes: vec![],
                            inner_html: Some(
                                r#"
                    <a href="https://hello.com">Hello</a>
                    <div>
                        <a href="https://world.com">World</a>
                    </div>
                "#
                            ),
                            text_content: Some("Hello World".to_string()),
                            children: HashMap::from([
                                (
                                    "div a",
                                    SelectionValue {
                                        kind: ValueKind::List,
                                        list: vec![Element {
                                            name: "a",
                                            id: None,
                                            class: None,
                                            attributes: vec![("href", Some("https://world.com"))],
                                            inner_html: Some("World"),
                                            text_content: Some("World".to_string()),
                                            children: HashMap::new(),
                                        },]
                                    }
                                ),
                                (
                                    "> a[href]",
                                    SelectionValue {
                                        kind: ValueKind::SingleItem,
                                        list: vec![Element {
                                            name: "a",
                                            id: None,
                                            class: None,
                                            attributes: vec![("href", Some("https://hello.com"))],
                                            inner_html: Some("Hello"),
                                            text_content: Some("Hello".to_string()),
                                            children: HashMap::new(),
                                        }]
                                    }
                                )
                            ]),
                        },
                        Element {
                            name: "section",
                            id: None,
                            class: None,
                            attributes: vec![],
                            inner_html: Some(
                                r#"
                    <a href="https://hello2.com">Hello2</a>

                    <div>
                        <a href="https://world2.com">World2</a>
                        <div>
                            <a href="https://world3.com">World3</a>
                        </div>
                    </div>
                "#
                            ),
                            text_content: Some("Hello2 World2 World3".to_string()),
                            children: HashMap::from([
                                (
                                    "div a",
                                    SelectionValue {
                                        kind: ValueKind::List,
                                        list: vec![
                                            Element {
                                                name: "a",
                                                id: None,
                                                class: None,
                                                attributes: vec![(
                                                    "href",
                                                    Some("https://world2.com")
                                                )],
                                                inner_html: Some("World2"),
                                                text_content: Some("World2".to_string()),
                                                children: HashMap::new(),
                                            },
                                            Element {
                                                name: "a",
                                                id: None,
                                                class: None,
                                                attributes: vec![(
                                                    "href",
                                                    Some("https://world3.com")
                                                )],
                                                inner_html: Some("World3"),
                                                text_content: Some("World3".to_string()),
                                                children: HashMap::new(),
                                            },
                                        ]
                                    }
                                ),
                                (
                                    "> a[href]",
                                    SelectionValue {
                                        kind: ValueKind::SingleItem,
                                        list: vec![Element {
                                            name: "a",
                                            id: None,
                                            class: None,
                                            attributes: vec![("href", Some("https://hello2.com"))],
                                            inner_html: Some("Hello2"),
                                            text_content: Some("Hello2".to_string()),
                                            children: HashMap::new(),
                                        },]
                                    }
                                )
                            ]),
                        },
                    ]
                }
            ),])
        );
    }

    const BASIC_HTML_WITH_SCRIPT: &str = r#"
        <html>
            <h1>Hello World</h1>

            <script>
                let x = 123132.2;
                let y = "<div>" + "Hello" + "</" + "div>";
            </script>
        </html>
        "#;

    #[test]
    fn test_script_tag_with_html_like_content() {
        let mut reader = Reader::new(BASIC_HTML_WITH_SCRIPT);

        let queries = &[Query::all("div", Save::none()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let map = parser.matches().root.children;
        //println!("Matches: {:#?}", map);
        assert_eq!(
            map,
            HashMap::from([(
                "div",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![],
                }
            )])
        )
    }

    const BASIC_HTML_WITH_SELF_CLOSING_TAG: &str = r#"
        <html>
            <h1>Hello World</h1>
            <form action="/my-handling-form-page" method="post">
                <p>
                    <label for="name">Name:</label>
                    <input type="text" id="name" name="user_name" />
                </p>
                <p>
                    <label for="mail">Email:</label>
                    <input type="email" id="mail" name="user_email" />
                </p>
                <p>
                    <label for="msg">Message:</label>
                    <textarea id="msg" name="user_message"></textarea>
                </p>
            </form>
        </html>
        "#;

    #[test]
    fn test_self_closing_tags() {
        let mut reader = Reader::new(BASIC_HTML_WITH_SELF_CLOSING_TAG);
        let queries = &[Query::all("form > p > input", Save::none()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let map = parser.matches().root.children;
        //println!("Matches: {:#?}", map);
        assert_eq!(
            map,
            HashMap::from([(
                "form > p > input",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![
                        Element {
                            name: "input",
                            id: Some("name"),
                            class: None,
                            attributes: vec![
                                ("type", Some("text")),
                                ("name", Some("user_name")),
                                ("/", None),
                            ],
                            inner_html: None,
                            text_content: None,
                            children: HashMap::new(),
                        },
                        Element {
                            name: "input",
                            id: Some("mail"),
                            class: None,
                            attributes: vec![
                                ("type", Some("email")),
                                ("name", Some("user_email")),
                                ("/", None),
                            ],
                            inner_html: None,
                            text_content: None,
                            children: HashMap::new(),
                        },
                    ],
                }
            )])
        )
    }

    #[test]
    fn test_self_closing_tags_with_content_query() {
        /*
         * What should happen?
         * Query Warning?
         * Handle it anyway?
         */
        let mut reader = Reader::new(BASIC_HTML_WITH_SELF_CLOSING_TAG);

        let queries = &[Query::all("form > p > input", Save::all()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let map = parser.matches().root.children;
        //println!("Matches: {:#?}", map);
        assert_eq!(
            map,
            HashMap::from([(
                "form > p > input",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![
                        Element {
                            name: "input",
                            id: Some("name"),
                            class: None,
                            attributes: vec![
                                ("type", Some("text")),
                                ("name", Some("user_name")),
                                ("/", None),
                            ],
                            inner_html: None,
                            text_content: None,
                            children: HashMap::new(),
                        },
                        Element {
                            name: "input",
                            id: Some("mail"),
                            class: None,
                            attributes: vec![
                                ("type", Some("email")),
                                ("name", Some("user_email")),
                                ("/", None),
                            ],
                            inner_html: None,
                            text_content: None,
                            children: HashMap::new(),
                        },
                    ],
                }
            )])
        )
    }

    const BASIC_ANCHOR_LIST: &str = r#"
        <a>Hello 1</a>
        <a>Hello 2</a>
        <a>Hello 3</a>
        "#;

    #[test]
    fn test_anchor_list_selection() {
        let mut reader = Reader::new(BASIC_ANCHOR_LIST);

        let queries = &[Query::all("a", Save::all()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let map = parser.matches().root.children;
        assert_eq!(
            map,
            HashMap::from([(
                "a",
                SelectionValue {
                    kind: ValueKind::List,
                    list: vec![
                        Element {
                            name: "a",
                            id: None,
                            class: None,
                            attributes: vec![],
                            inner_html: Some("Hello 1"),
                            text_content: Some("Hello 1".to_string()),
                            children: HashMap::new(),
                        },
                        Element {
                            name: "a",
                            id: None,
                            class: None,
                            attributes: vec![],
                            inner_html: Some("Hello 2"),
                            text_content: Some("Hello 2".to_string()),
                            children: HashMap::new(),
                        },
                        Element {
                            name: "a",
                            id: None,
                            class: None,
                            attributes: vec![],
                            inner_html: Some("Hello 3"),
                            text_content: Some("Hello 3".to_string()),
                            children: HashMap::new(),
                        },
                    ],
                }
            )])
        )
    }

    const POSTS: &str = r#"<div class="article"><a href="/post/0"><b>Post</b> &lt;0&gt;</a></div><div class="article"><a href="/post/1"><b>Post</b> &lt;1&gt;</a></div>"#;

    #[test]
    fn test_first_anchor_in_list_selection() {
        let mut reader = Reader::new(POSTS);

        let queries = &[Query::first("div.article a", Save::all()).build()];

        let manager = FsmManager::new(RustStore::new(false), queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let map = parser.matches().root.children;
        assert_eq!(
            map,
            HashMap::from([(
                "div.article a",
                SelectionValue {
                    kind: ValueKind::SingleItem,
                    list: vec![Element {
                        name: "a",
                        id: None,
                        class: None,
                        attributes: vec![("href", Some("/post/0"))],
                        inner_html: Some("<b>Post</b> &lt;0&gt;"),
                        text_content: Some("Post &lt;0&gt;".to_string()),
                        children: HashMap::new(),
                    },],
                }
            )])
        )
    }
}
