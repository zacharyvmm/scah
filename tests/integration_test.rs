mod tests {
    use std::collections::HashMap;

    use onego::{
        Element, QueryBuilder, QueryError, Save, SelectionKind, SelectionPart, SelectionValue,
        parse,
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
        let selection_tree = QueryBuilder::new(section);

        let queries = &[selection_tree.build()];
        let map = parse(HTML, queries);

        let list = &map["main > section#id"];
        println!("{:#?}", list);

        let last = &list[list.len()? - 1];

        assert!(last.inner_html.is_some());
        assert_eq!(
            last.inner_html.unwrap().trim(),
            r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
        );

        assert!(last.text_content.is_some());
        assert_eq!(
            last.text_content.clone().unwrap(),
            r#"Not selected (main has no red-background class)"#
        );

        let first = &list[0];
        assert_eq!(
            first.inner_html.unwrap().trim(),
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
            first.text_content.clone().unwrap(),
            r#"Link 1 Link 2 Link 3 Not selected (nested in div) No link here"#
        );

        Ok(())
    }

    #[test]
    fn test_html_page_all_anchor_tag_selection<'key>() -> Result<(), QueryError<'key>> {
        let queries = &[QueryBuilder::new(SelectionPart::new(
            "a",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))
        .build()];
        let map = parse(HTML, queries);

        assert_eq!(map["a"].len()?, 7);
        println!("{:#?}", map);
        Ok(())
    }

    #[test]
    fn test_html_page_first_anchor_tag_selection<'key>() -> Result<(), QueryError<'key>> {
        let queries = &[QueryBuilder::new(SelectionPart::new(
            "a",
            SelectionKind::First(Save {
                inner_html: true,
                text_content: true,
            }),
        ))
        .build()];
        let map = parse(HTML, queries);

        assert_eq!(map["a"].len()?, 1);
        println!("{:#?}", map);
        Ok(())
    }

    #[test]
    fn test_html_page_all_anchor_tag_starting_with_link_selection<'key>()
    -> Result<(), QueryError<'key>> {
        let queries = &[QueryBuilder::new(SelectionPart::new(
            "a[href^=link]",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        )).build()];
        let map = parse(HTML, queries);

        assert_eq!(map["a[href^=link]"].len()?, 3);
        Ok(())
    }

    #[test]
    fn test_html_page_children_valid_anchor_tags_in_main<'key>() -> Result<(), QueryError<'key>> {
        let queries = &[QueryBuilder::new(SelectionPart::new(
            "main > section > a[href]",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ))
        .build()];
        let map = parse(HTML, queries);

        assert_eq!(map["main > section > a[href]"].len()?, 5);
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

        let selection_tree = QueryBuilder::new(section);

        let queries = &[selection_tree.build()];
        let map = parse(HTML, queries);

        assert_eq!(map["main.red-background > section#id"].len()?, 1);
        Ok(())
    }

    #[test]
    fn test_html_multi_selection<'key>() -> Result<(), QueryError<'key>> {
        let mut queries = QueryBuilder::new(SelectionPart::new(
            "main > section",
            SelectionKind::All(Save {
                inner_html: true,
                text_content: true,
            }),
        ));
        queries.append(vec![
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

        let q = [queries.build()];
        let map = parse(HTML, &q);

        println!("Map: {:#?}", map);
        Ok(())
    }
}
