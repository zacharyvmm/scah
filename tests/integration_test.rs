use onego::{Attribute, Element, Query, QueryError, Save, parse};
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
    let selection_tree = Query::all("main > section#id", Save::all());

    let queries = &[selection_tree.build()];
    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let indices = root["main > section#id"].iter().unwrap();
    let mut list = indices.map(|i| &store.arena[*i]);

    println!("{:#?}", list);

    let last = list.clone().last().unwrap();

    assert!(last.inner_html.is_some());
    assert_eq!(
        last.inner_html.unwrap().trim(),
        r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
    );

    assert!(last.text_content.is_some());
    assert_eq!(
        store.text_content(last).unwrap(),
        r#"Not selected (main has no red-background class)"#
    );

    let first = list.next().unwrap();
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
        store.text_content(first).unwrap(),
        r#"Link 1 Link 2 Link 3 Not selected (nested in div) No link here"#
    );

    Ok(())
}

#[test]
fn test_html_page_all_anchor_tag_selection<'key>() -> Result<(), QueryError<'key>> {
    let queries = &[Query::all("a", Save::all()).build()];
    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let indices = root["a"].iter().unwrap();
    let list = indices.map(|i| &store.arena[*i]);

    assert_eq!(list.clone().count(), 7);
    println!("List: {:#?}", list.collect::<Vec<_>>());
    Ok(())
}

#[test]
fn test_html_page_first_anchor_tag_selection<'key>() -> Result<(), QueryError<'key>> {
    let queries = &[Query::first("a", Save::all()).build()];
    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let child = &root["a"];

    assert_eq!(
        store.arena[child.value()?],
        Element {
            name: "a",
            class: None,
            id: None,
            attributes: vec![Attribute {
                key: "href",
                value: Some("link1")
            }],
            inner_html: Some("Link 1"),
            text_content: Some(0..6),
            children: Vec::new(),
        }
    );
    Ok(())
}

#[test]
fn test_html_page_all_anchor_tag_starting_with_link_selection<'key>() -> Result<(), QueryError<'key>>
{
    let queries = &[Query::all("a[href^=link]", Save::all()).build()];
    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let indices = root["a[href^=link]"].iter().unwrap();
    let list = indices.map(|i| &store.arena[*i]);

    assert_eq!(list.count(), 3);
    Ok(())
}

#[test]
fn test_html_page_children_valid_anchor_tags_in_main<'key>() -> Result<(), QueryError<'key>> {
    let queries = &[Query::all("main > section > a[href]", Save::all()).build()];

    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let indices = root["main > section > a[href]"].iter().unwrap();
    let list = indices.map(|i| &store.arena[*i]);

    assert_eq!(list.count(), 5);
    Ok(())
}

#[test]
fn test_html_page_single_main<'key>() -> Result<(), QueryError<'key>> {
    let queries = &[Query::all("main.red-background > section#id", Save::all()).build()];
    let store = parse(HTML, queries);
    let root = &store.arena[0];

    let indices = root["main.red-background > section#id"].iter().unwrap();
    let list = indices.map(|i| &store.arena[*i]);

    assert_eq!(list.count(), 1);
    Ok(())
}

#[test]
fn test_html_multi_selection<'key>() -> Result<(), QueryError<'key>> {
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
    let root = &store.arena[0];

    let indices = root["main > section"].iter().unwrap();
    let list = indices.map(|i| &store.arena[*i]);

    println!("List: {:#?}", list.collect::<Vec<_>>());
    Ok(())
}
