mod tests {
    use std::collections::HashMap;

    use onego::{
        Element, QueryError, Save, Selection, SelectionKind, SelectionPart, SelectionValue, parse,
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
    fn test_html_page<'key>() -> Result<(), QueryError<'key>> {
        let section = SelectionPart::new(
            "main > section#id",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        );
        let selection_tree = Selection::new(section);

        let queries = &vec![selection_tree];
        let map = parse(HTML, queries);

        let list = &map["main > section#id"];

        let last = &list[list.len()? - 1];

        // assert!(last.inner_html.is_some());
        // assert_eq!(
        //     last.inner_html.unwrap().trim(),
        //     r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
        // );

        // assert!(last.text_content.is_some());
        // assert_eq!(
        //     last.text_content.unwrap(),
        //     r#"Not selected (main has no red-background class)"#
        // );

        // let first = &list[0];
        // assert_eq!(
        //     first.inner_html.unwrap().trim(),
        //     r#"<!-- These 3 links will be selected by the selector -->
        //         <a href="link1">Link 1</a>
        //         <a href="link2">Link 2</a>
        //         <a href="link3">Link 3</a>

        //         <!-- These elements won't be selected -->
        //         <div>
        //             <a href="not-selected">Not selected (nested in div)</a>
        //         </div>
        //         <span>No link here</span>"#
        // );

        // assert_eq!(
        //     first.text_content.unwrap(),
        //     r#"Link 1 Link 2 Link 3 Not selected (nested in div) No link here"#
        // );

        Ok(())
    }
    #[test]
    fn test_html_page_single_main<'key>() -> Result<(), QueryError<'key>> {
        let section = SelectionPart::new(
            "main.red-background > section#id",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        );

        let selection_tree = Selection::new(section);

        let queries = &vec![selection_tree];
        let map = parse(HTML, queries);

        assert_eq!(map["main.red-background > section#id"].len()?, 1);
        Ok(())
    }

    #[test]
    fn test_html_page_all_anchor_tag_selection<'key>() -> Result<(), QueryError<'key>> {
        let queries = &vec![Selection::new(SelectionPart::new(
            "a",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))];
        let map = parse(HTML, queries);

        assert_eq!(map["a"].len()?, 7);
        Ok(())
    }

    #[test]
    fn test_html_page_all_anchor_tag_starting_with_link_selection<'key>()
    -> Result<(), QueryError<'key>> {
        let queries = &vec![Selection::new(SelectionPart::new(
            "a[href^=link]",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))];
        let map = parse(HTML, queries);

        assert_eq!(map["a[href^=link]"].len()?, 3);
        Ok(())
    }

    #[test]
    fn test_html_page_children_valid_anchor_tags_in_main<'key>() -> Result<(), QueryError<'key>> {
        let queries = &vec![Selection::new(SelectionPart::new(
            "main > section > a[href]",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))];
        let map = parse(HTML, queries);

        assert_eq!(map["main > section > a[href]"].len()?, 5);
        Ok(())
    }

    #[test]
    fn test_html_multi_selection<'key>() -> Result<(), QueryError<'key>> {
        let mut queries = vec![Selection::new(SelectionPart::new(
            "main > section",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))];
        queries[0].append(vec![
            SelectionPart::new(
                "> a[href]",
                SelectionKind::First(Save {
                    inner_html: true,
                    text_content: true,
                }),
            ),
            SelectionPart::new(
                "div a",
                SelectionKind::All(Save {
                    inner_html: true,
                    text_content: true,
                }),
            ),
        ]);

        let map = parse(HTML, &queries);

        println!("{:?}", map);
        Ok(())
    }

    /*
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
    } */
}
