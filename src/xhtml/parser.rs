use std::fmt::Debug;

use super::element::element::XHtmlTag;
use super::text_content::TextContent;
use crate::XHtmlElement;
use crate::css::DocumentPosition;
use crate::css::FsmManager;
use crate::dbg_print;
use crate::store::Store;
use crate::utils::Reader;

pub struct XHtmlParser<'html, 'query> {
    position: DocumentPosition,
    pub selectors: FsmManager<'query>,
    store: Store<'html, 'query>,
    element: crate::XHtmlElement<'html>,
    in_script: bool,
}

impl<'html, 'query: 'html> XHtmlParser<'html, 'query> {
    pub fn new(selectors: FsmManager<'query>) -> Self {
        Self {
            position: DocumentPosition {
                element_depth: 0,
                reader_position: 0, // for inner_html
                text_content_position: usize::MAX,
            },
            selectors,
            element: XHtmlElement::new(),
            in_script: false,
            store: Store::new(),
        }
    }

    pub fn with_capacity(selectors: FsmManager<'query>, capacity: usize) -> Self {
        Self {
            position: DocumentPosition {
                element_depth: 0,
                reader_position: 0, // for inner_html
                text_content_position: usize::MAX,
            },
            selectors,
            element: XHtmlElement::new(),
            in_script: false,
            store: Store::with_capacity(capacity),
        }
    }

    pub fn next(&mut self, reader: &mut Reader<'html>) -> bool {
        if self.in_script {
            loop {
                reader.next_until(b'<');
                if reader.peek().is_none() {
                    return false;
                }

                if reader.match_ignore_case("</script>") {
                    if self.store.text_content.text_start.is_some() {
                        if let Some(position) =
                            self.store.text_content.push(reader, reader.get_position())
                        {
                            self.position.text_content_position = position;
                        }
                    }
                    self.in_script = false;
                    break;
                } else {
                    reader.skip();
                }
            }
        }

        // move until it finds the first `<`
        reader.next_until(b'<');

        if reader.peek().is_none() {
            return false;
        }

        let tag = {
            let mut tag: Option<XHtmlTag> = None;

            while tag.is_none() {
                self.position.reader_position = reader.get_position();
                tag = XHtmlTag::from(reader);
                if let Some(XHtmlTag::Open) = tag {
                    self.element.from(reader);
                } else if tag.is_none()
                    && self.store.text_content.text_start.is_some()
                    && let Some(position) = self
                        .store
                        .text_content
                        .push(reader, self.position.reader_position)
                {
                    self.position.text_content_position = position;
                    self.store.text_content.set_start(reader.get_position());
                }
            }

            tag.unwrap()
        };

        if self.store.text_content.text_start.is_some()
            && let Some(position) = self
                .store
                .text_content
                .push(reader, self.position.reader_position)
        {
            self.position.text_content_position = position;
        }

        self.store.text_content.set_start(reader.get_position());

        // TODO: register the start
        //reader.next_while(|c| c.is_whitespace());
        let mut early_exit = false;

        match tag {
            XHtmlTag::Open => {
                if self.element.name.eq_ignore_ascii_case("script") {
                    self.in_script = true;
                }

                self.position.element_depth += 1;
                self.position.reader_position = reader.get_position();

                dbg_print!(
                    "opening: `{}` ({})",
                    self.element.name,
                    self.position.element_depth
                );

                let mut remove_depth_after_next = false;
                if self.element.is_self_closing() {
                    remove_depth_after_next = true;
                }

                self.selectors
                    .next(&self.element, &self.position, &mut self.store);

                if remove_depth_after_next {
                    self.position.element_depth -= 1;
                }

                self.element.clear();
            }
            XHtmlTag::Close(closing_tag) => {
                dbg_print!("closing: `{closing_tag}` ({})", self.position.element_depth);

                early_exit =
                    self.selectors
                        .back(closing_tag, &self.position, reader, &mut self.store);
                self.position.element_depth -= 1;
            }
        }

        !early_exit && !reader.eof()
    }

    pub fn matches(self) -> Store<'html, 'query> {
        self.store
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Attribute;
    use crate::css::{FsmManager, Query, Save};
    use crate::store::{Child, ChildIndex};
    use crate::store::{Element, Store};
    use crate::utils::Reader;
    use pretty_assertions::assert_eq;

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

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let store = parser.matches();
        let root = &store.elements[0];

        assert_eq!(root.name, "root");
        assert_eq!(root.children.len(), 1);
        let child_node = &root.children[0];
        assert_eq!(child_node.query, "p.indent > .bold");

        let indices = match &child_node.index {
            ChildIndex::Many(indices) => indices,
            _ => panic!("Expected list"),
        };
        assert_eq!(indices.len(), 1);
        let span = &store.elements[indices[0]];

        assert_eq!(span.name, "span");
        assert_eq!(span.id, Some("name"));
        assert_eq!(span.class, Some("bold"));
    }

    #[test]
    fn test_text_content() {
        let mut reader = Reader::new(BASIC_HTML);

        let queries = &[Query::all("p.indent > .bold", Save::none()).build()];
        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        let mut continue_parser = parser.next(&mut reader); // <html>
        assert!(continue_parser);

        continue_parser = parser.next(&mut reader); // <h1>
        assert!(continue_parser);

        continue_parser = parser.next(&mut reader); // </h1>
        assert!(continue_parser);
        assert_eq!(parser.store.text_content.content, b"Hello World ");

        continue_parser = parser.next(&mut reader); // <p class="indent">
        assert!(continue_parser);
        assert_eq!(parser.store.text_content.content, b"Hello World ");

        continue_parser = parser.next(&mut reader); // <span id="name" class="bold">
        assert!(continue_parser);
        assert_eq!(
            parser.store.text_content.content,
            b"Hello World My name is "
        );

        continue_parser = parser.next(&mut reader); // </span>
        assert!(continue_parser);
        assert_eq!(
            parser.store.text_content.content,
            b"Hello World My name is Zachary "
        );

        continue_parser = parser.next(&mut reader); // </p>
        assert!(continue_parser);
        assert_eq!(
            parser.store.text_content.content,
            b"Hello World My name is Zachary "
        );

        continue_parser = parser.next(&mut reader); // </html>
        assert!(!continue_parser);
        assert_eq!(
            parser.store.text_content.content,
            b"Hello World My name is Zachary "
        );
    }

    #[test]
    fn test_top_level_multi_selection() {
        let mut reader = Reader::new(BASIC_HTML);

        let queries = &[
            Query::all("p.indent > .bold", Save::none()).build(),
            Query::all("h1 + .indent #name", Save::none()).build(),
        ];

        let manager = FsmManager::new(queries);

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
                section.all("> a[href]", Save::all()),
                section.all("div a", Save::all()),
            ]
        });
        let queries = &[queries.build()];
        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let root = &store.elements[0];

        // main > section
        let sections_idx = &root["main > section"];
        let sections: Vec<&Element> = sections_idx
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(sections.len(), 2);

        // Section 1
        let s1 = sections[0];
        assert_eq!(store.text_content(s1), Some("Hello World"));

        let s1_div_a: Vec<&Element> = s1["div a"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(s1_div_a.len(), 1);
        assert_eq!(store.text_content(s1_div_a[0]), Some("World"));
        assert_eq!(s1_div_a[0].attributes[0].value, Some("https://world.com"));

        let s1_direct_a: Vec<&Element> = s1["> a[href]"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(s1_direct_a.len(), 1);
        assert_eq!(store.text_content(s1_direct_a[0]), Some("Hello"));
        assert_eq!(
            s1_direct_a[0].attributes[0].value,
            Some("https://hello.com")
        );

        // Section 2
        let s2 = sections[1];
        assert_eq!(store.text_content(s2), Some("Hello2 World2 World3"));

        let s2_div_a: Vec<&Element> = s2["div a"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(s2_div_a.len(), 2);
        assert_eq!(store.text_content(s2_div_a[0]), Some("World2"));
        assert_eq!(store.text_content(s2_div_a[1]), Some("World3"));

        let s2_direct_a: Vec<&Element> = s2["> a[href]"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(s2_direct_a.len(), 1);
        assert_eq!(store.text_content(s2_direct_a[0]), Some("Hello2"));
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

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let store = parser.matches();
        let root = &store.elements[0];

        // It should NOT find any div
        if let Ok(div_idx) = root.get("div") {
            assert_eq!(div_idx.iter().unwrap().count(), 0);
        }
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

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        println!("{:?}", queries);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let root = &store.elements[0];

        let inputs: Vec<&Element> = root["form > p > input"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(inputs.len(), 2);

        assert_eq!(inputs[0].name, "input");
        assert_eq!(inputs[0].id, Some("name"));
        assert_eq!(inputs[0].attributes[0].key, "type");
        assert_eq!(inputs[0].attributes[0].value, Some("text"));

        assert_eq!(inputs[1].name, "input");
        assert_eq!(inputs[1].id, Some("mail"));
        assert_eq!(inputs[1].attributes[0].key, "type");
        assert_eq!(inputs[1].attributes[0].value, Some("email"));
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

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        // STEP 1
        //let mut continue_parser = parser.next(&mut reader);

        println!("{:?}", queries);

        while parser.next(&mut reader) {
            // println!("{:?}", parser.selectors);
        }

        let store = parser.matches();
        let root = &store.elements[0];

        let inputs: Vec<&Element> = root["form > p > input"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].text_content, None);
        assert_eq!(inputs[0].inner_html, None);

        assert_eq!(inputs[1].text_content, None);
        assert_eq!(inputs[1].inner_html, None);
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

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let root = &store.elements[0];

        let anchors: Vec<&Element> = root["a"]
            .iter()
            .unwrap()
            .map(|i| &store.elements[*i])
            .collect();
        assert_eq!(anchors.len(), 3);

        assert_eq!(store.text_content(anchors[0]), Some("Hello 1"));
        assert_eq!(store.text_content(anchors[1]), Some("Hello 2"));
        assert_eq!(store.text_content(anchors[2]), Some("Hello 3"));
    }

    const POSTS: &str = r#"<div class="article"><a href="/post/0"><b>Post</b> &lt;0&gt;</a></div><div class="article"><a href="/post/1"><b>Post</b> &lt;1&gt;</a></div>"#;

    #[test]
    fn test_first_anchor_in_list_selection() {
        let mut reader = Reader::new(POSTS);

        let queries = &[Query::first("div.article a", Save::all()).build()];

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let root = &store.elements[0];

        let anchor_idx = root.get("div.article a").unwrap().value().unwrap();
        let anchor = &store.elements[anchor_idx];

        assert_eq!(anchor.name, "a");
        assert_eq!(anchor.attributes[0].value, Some("/post/0"));
        assert_eq!(anchor.inner_html, Some("<b>Post</b> &lt;0&gt;"));
        assert_eq!(store.text_content(anchor), Some("Post &lt;0&gt;"));
    }

    const PYTHON_TEST_HTML: &str = r#"
    <span class="hello" id="world" hello="world">
        Hello <a href="https://www.example.com">World</a>
    </span>
    <p class="example_class" id="example_id" hello="example">
        My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
    </p>
    "#;

    #[test]
    fn test_python_test_html() {
        let mut reader = Reader::new(PYTHON_TEST_HTML);

        let queries = &[Query::all("#world", Save::all())
            .all("a", Save::all())
            .build()];

        // assert_eq!(queries, &[Query {
        //     queries: vec![].into_boxed_slice(),
        //     states: vec![].into_boxed_slice(),
        //     exit_at_section_end: None,
        // }]);

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let store = parser.matches();

        assert_eq!(
            store.elements,
            vec![
                Element {
                    name: "root",
                    class: None,
                    id: None,
                    attributes: vec![],
                    inner_html: None,
                    text_content: None,
                    children: vec![Child {
                        query: "#world",
                        index: ChildIndex::Many(vec![1])
                    }],
                },
                Element {
                    name: "span",
                    class: Some("hello"),
                    id: Some("world"),
                    attributes: vec![Attribute {
                        key: "hello",
                        value: Some("world")
                    }],
                    inner_html: Some(
                        r#"
        Hello <a href="https://www.example.com">World</a>
    "#
                    ),
                    text_content: store.elements[1].text_content.clone(),
                    children: vec![Child {
                        query: "a",
                        index: ChildIndex::Many(vec![2])
                    }],
                },
                Element {
                    name: "a",
                    class: None,
                    id: None,
                    attributes: vec![Attribute {
                        key: "href",
                        value: Some("https://www.example.com")
                    }],
                    inner_html: Some("World"),
                    text_content: store.elements[2].text_content.clone(),
                    children: vec![],
                },
            ]
        );
    }

    #[test]
    fn test_first_anchor_tag_from_bench() {
        fn generate_html(count: usize) -> String {
            let mut html = String::with_capacity(count * 100);
            html.push_str("<html><body><div id='content'>");
            for i in 0..count {
                // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
                html.push_str(&format!(
                    r#"<div class="article"><a href="/post/{}"><b>Post</b> &lt;{}&gt;</a></div>"#,
                    i, i
                ));
            }
            html.push_str("</div></body></html>");
            html
        }

        let html = generate_html(100);
        let mut reader = Reader::from_bytes(html.as_bytes());

        let query = Query::first("a", Save::all()).build();
        assert_eq!(query.exit_at_section_end, Some(0));
        let queries = &[query];

        let manager = FsmManager::new(queries);

        let mut parser = XHtmlParser::new(manager);

        while parser.next(&mut reader) {}

        let store = parser.matches();
        let root = &store.elements[0];

        let element_index = root["a"].value().unwrap();
        let element = &store.elements[element_index];

        assert_eq!(element.attributes, vec![Attribute {
            key: "href",
            value: Some("/post/0"),
        }]);

        assert_eq!(element.inner_html, Some("<b>Post</b> &lt;0&gt;"));
        assert_eq!(store.text_content(&element), Some("Post &lt;0&gt;"));
    }
}
