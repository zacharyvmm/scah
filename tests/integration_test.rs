/*use onego::{
    Attribute, BodyContent, InnerContent, SelectorQuery, SelectorQueryKind, XHtmlElement, parse,
};
const HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
    <style>
        .red-background {
            background-color: #ffdddd;
        }
    </style>
</head>
<body>
    <main class="red-background">
        <section id="id">
            <!-- These 3 links will be selected by the selector -->
            <a href="link1">Link 1</a>
            <a href="link2">Link 2</a>
            <a href="link3">Link 3</a>
            
            <!-- These elements won't be selected -->
            <div>
                <a href="not-selected">Not selected (nested in div)</a>
            </div>
            <span>No link here</span>
        </section>
        
        <!-- These elements won't be selected -->
        <section>
            <a href="wrong-section">Not selected (wrong section)</a>
        </section>
        <a href="direct-link">Not selected (direct child of main)</a>
    </main>
    
    <!-- These elements won't be selected -->
    <main>
        <section id="id" class="third-section">
            <a href="wrong-main">Not selected (main has no red-background class)</a>
        </section>
    </main>
</body>
</html>
"#;

#[test]
fn test_html_page() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "main > section#id",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (mut map, content) = parse(HTML, queries);

    let last = map.elements.pop().unwrap();
    assert_eq!(
        HTML[last.inner_html.unwrap()].trim(),
        r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
    );

    assert_eq!(
        content.join(last.text_content.unwrap()).trim(),
        r#"Not selected (main has no red-background class)"#
    );

    let first = map.elements.pop().unwrap();
    assert_eq!(
        HTML[first.inner_html.unwrap()].trim(),
        r#"<!-- These 3 links will be selected by the selector -->
            <a href="link1">Link 1</a>
            <a href="link2">Link 2</a>
            <a href="link3">Link 3</a>
            
            <!-- These elements won't be selected -->
            <div>
                <a href="not-selected">Not selected (nested in div)</a>
            </div>
            <span>No link here</span>"#
    );

    assert_eq!(
        content.join(first.text_content.unwrap()).trim(),
        r#"Link 1 Link 2 Link 3 Not selected (nested in div) No link here"#
    );
}

#[test]
fn test_html_page_single_main() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "main.red-background > section#id",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (map, _) = parse(HTML, queries);

    assert_eq!(map.elements.len(), 1);
}

#[test]
fn test_html_page_all_anchor_tag_selection() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "a",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (map, _) = parse(HTML, queries);

    assert_eq!(map.elements.len(), 7);
}

#[test]
fn test_html_page_all_anchor_tag_starting_with_link_selection() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "a[href^=link]",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (map, _) = parse(HTML, queries);

    assert_eq!(map.elements.len(), 3);
}

#[test]
fn test_html_page_children_valid_anchor_tags_in_main() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "main > section > a[href]",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (map, _) = parse(HTML, queries);

    assert_eq!(map.elements.len(), 5);
}

#[test]
fn test_html_page_all_valid_anchor_tags_in_main() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "main > section a[href]",
        data: InnerContent {
            inner_html: true,
            text_content: true,
        },
    }]);

    let (mut map, content) = parse(HTML, queries);
    // I think the bug is that the on pop it's still on the a[href]
    // thus if the current element has a depth of 0 we need to check the element before that
    assert_eq!(map.elements.len(), 6);
    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[0];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("link1")
                }])
            }
        );

        assert_eq!(content.join(text_content.clone().unwrap()), "Link 1");
        assert_eq!(&HTML[inner_html.clone().unwrap()], "Link 1");
    }

    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[1];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("link2")
                }])
            }
        );

        assert_eq!(content.join(text_content.clone().unwrap()), "Link 2");
        assert_eq!(&HTML[inner_html.clone().unwrap()], "Link 2");
    }

    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[2];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("link3")
                }])
            }
        );

        assert_eq!(content.join(text_content.clone().unwrap()), "Link 3");
        assert_eq!(&HTML[inner_html.clone().unwrap()], "Link 3");
    }

    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[3];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("not-selected")
                }])
            }
        );

        assert_eq!(
            content.join(text_content.clone().unwrap()),
            "Not selected (nested in div)"
        );
        assert_eq!(
            &HTML[inner_html.clone().unwrap()],
            "Not selected (nested in div)"
        );
    }

    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[4];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("wrong-section")
                }])
            }
        );

        assert_eq!(
            content.join(text_content.clone().unwrap()),
            "Not selected (wrong section)"
        );
        assert_eq!(
            &HTML[inner_html.clone().unwrap()],
            "Not selected (wrong section)"
        );
    }

    {
        let BodyContent {
            element,
            inner_html,
            text_content,
        } = &map.elements[5];
        assert_eq!(
            *element,
            XHtmlElement {
                name: "a",
                id: None,
                class: None,
                attributes: Vec::from([Attribute {
                    name: "href",
                    value: Some("wrong-main")
                }])
            }
        );

        assert_eq!(
            content.join(text_content.clone().unwrap()),
            "Not selected (main has no red-background class)"
        );
        assert_eq!(
            &HTML[inner_html.clone().unwrap()],
            "Not selected (main has no red-background class)"
        );
    }
}*/
