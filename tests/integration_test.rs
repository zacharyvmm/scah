use scrooge::{InnerContent, SelectorQuery, SelectorQueryKind, parse};

#[test]
fn test_html_page() {
    let queries = Vec::from([SelectorQuery {
        kind: SelectorQueryKind::All,
        query: "main.red-background > section#id",
        data: InnerContent {
            inner_html: true,
            text_content: true,
            //attributes: Vec::from(["href"]),
        },
    }]);

    let html = r#"
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

    let map = parse(html, queries);
    println!("{:?}", map);

    assert!(false);
}
