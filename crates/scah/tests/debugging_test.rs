use scah::{Query, Save, parse};

// TDD

const HTML_SCOPE_PROBLEM_INTRO_EXAMPLE: &str = r#"
    <div id="project-intro">
        <header>
            <h1 class="title">scah: Streamlined CSS-Selector HTML Extraction</h1>
            <p class="subtitle">A high-performance parsing library built as a bachelor's thesis project.</p>
        </header>
        <article class="overview">
            <p><strong>scah</strong> (<em>scan HTML</em>) bridges the gap between SAX/StAX streaming efficiency and DOM convenience.</p>
            <p>Instead of manually tracking parser state or loading massive documents into memory, you declare your extraction targets using standard CSS selectors.</p>
        </article>

        <aside class="ecosystem">
            <h3>Language Bindings</h3>
            <ul>
                <li class="existing">Python</li>
                <li class="existing">Node.js</li>
                <li class="planned">Unified C API</li>
            </ul>
        </aside>
    </div>
"#;

#[test]
fn html_scope_problem_intro_example() {
    let queries = [Query::all("div#project-intro", Save::all())
        .unwrap()
        .then(|intro| {
            Ok([
                intro.all("article.overview p", Save::all())?,
                intro.all("aside.ecosystem li.existing", Save::all())?,
            ])
        })
        .unwrap()
        .build()];

    let store = parse(HTML_SCOPE_PROBLEM_INTRO_EXAMPLE, &queries);
    let intro = store.get("div#project-intro").unwrap().next().unwrap();

    let overview_paragraphs = intro
        .get(&store, "article.overview p")
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(overview_paragraphs.len(), 2);
    assert_eq!(overview_paragraphs[0].name, "p");
    assert_eq!(overview_paragraphs[1].name, "p");

    let existing_bindings = intro
        .get(&store, "aside.ecosystem li.existing")
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(existing_bindings.len(), 2);
    assert_eq!(existing_bindings[0].text_content(&store), Some("Python"));
    assert_eq!(existing_bindings[1].text_content(&store), Some("Node.js"));
}
