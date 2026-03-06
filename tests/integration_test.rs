use scah::{Attribute, Element, Query, Save, parse};
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
    let selection_tree = Query::all("main > section#id", Save::all());

    let queries = &[selection_tree.build()];
    let store = parse(HTML, queries);
    let list = store.get("main > section#id").unwrap().collect::<Vec<_>>();

    println!("{:#?}", list);

    let last = list.last().unwrap();

    assert!(last.inner_html.is_some());
    assert_eq!(
        last.inner_html.unwrap().trim(),
        r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
    );

    assert!(last.text_content(&store).is_some());
    assert_eq!(
        last.text_content(&store).unwrap(),
        r#"Not selected (main has no red-background class)"#
    );

    let first = list.iter().next().unwrap();
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
        first.text_content(&store).unwrap(),
        r#"Link 1 Link 2 Link 3 Not selected (nested in div) No link here"#
    );
}

#[test]
fn test_html_page_all_anchor_tag_selection() {
    let queries = &[Query::all("a", Save::all()).build()];
    let store = parse(HTML, queries);
    let root = &store.elements[0];
    println!("Strore: {:#?}", store);

    let list = store.get("a").unwrap().collect::<Vec<_>>();

    assert_eq!(list.iter().count(), 7);
    println!("List: {:#?}", list);
}

#[test]
fn test_html_page_first_anchor_tag_selection() {
    let queries = &[Query::first("a", Save::all()).build()];
    let store = parse(HTML, queries);
    let mut children = store.get("a").unwrap();

    let a = children.next().unwrap();
    assert_eq!(
        store.attributes,
        vec![Attribute {
            key: "href",
            value: Some("link1")
        }]
    );
    assert_eq!(a.name, "a");
    assert_eq!(
        a.attributes(&store).unwrap(),
        &[Attribute {
            key: "href",
            value: Some("link1")
        }]
    );
    assert_eq!(
        a.attribute(&store, "href"),
        Some("link1")
    );
    assert_eq!(a.text_content(&store).unwrap(), "Link 1");
}

#[test]
fn test_html_page_all_anchor_tag_starting_with_link_selection() {
    let queries = &[Query::all("a[href^=link]", Save::all()).build()];
    let store = parse(HTML, queries);
    let list = store.get("a[href^=link]").unwrap();

    assert_eq!(list.count(), 3);
}

#[test]
fn test_html_page_children_valid_anchor_tags_in_main() {
    let queries = &[Query::all("main > section > a[href]", Save::all()).build()];

    let store = parse(HTML, queries);
    let list = store.get("main > section > a[href]").unwrap();

    assert_eq!(list.count(), 5);
}

#[test]
fn test_html_page_single_main() {
    let queries = &[Query::all("main.red-background > section#id", Save::all()).build()];
    let store = parse(HTML, queries);
    let list = store.get("main.red-background > section#id").unwrap();

    assert_eq!(list.count(), 1);
}

#[test]
fn test_html_multi_selection() {
    let query = Query::all("main > section", Save::all())
        .then(|section| {
            [
                // BUG: first selection not working because their is no locking mechanism
                //section.first("> a[href]", Save::all()),
                section.all("> a[href]", Save::all()),
                section.all("div a", Save::all()),
                // BUG: If their are 2 identical sub-queries their should be an error.
                //section.all("> a[href]", Save::all()),
            ]
        })
        .build();

    let q = &[query];
    let store = parse(HTML, q);
    let list = store.get("main > section").unwrap();

    println!("List: {:#?}", list.collect::<Vec<_>>());
}
