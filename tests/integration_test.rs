use scrooge::{BodyContent, InnerContent, SelectorQuery, SelectorQueryKind, parse};
const html: &str = r#"
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

    let (mut map, content) = parse(html, queries);

    let last = map.elements.pop().unwrap();
    assert_eq!(
        html[last.inner_html.unwrap()].trim(),
        r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
    );

    assert_eq!(
        content.join(last.text_content.unwrap()).trim(),
        r#"Not selected (main has no red-background class)"#
    );

    let first = map.elements.pop().unwrap();
    assert_eq!(
        html[first.inner_html.unwrap()].trim(),
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

    let (map, _) = parse(html, queries);

    // This fails because the statemachine does not step back
    assert_eq!(map.elements.len(), 1);
}