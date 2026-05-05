use scah::{Query, Save, parse, query};

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

const FIRST_SELECTION_AS_ROOT_EARLY_EXIT: &str = r#"
    <div class="product"><h1>Product 0</h1><span class="rating">3/5</span><p class="description">Description</p></div>
    <div class="product"><h1>Product 1</h1><span class="rating">3/5</span><p class="description">Description</p></div>
"#;

#[test]
fn first_selection_as_root_early_exit() {
    const PRODUCT_SELECTOR: &str = "div.product";
    const PRODUCT_TITLE_SELECTOR: &str = "> h1";
    const PRODUCT_RATING_SELECTOR: &str = "> span.rating";
    const PRODUCT_DESCRIPTION_SELECTOR: &str = "> p.description";

    let queries = &[query! {
        first("div.product", Save::all()) => {
            first("> h1", Save::all()),
            first("> span.rating", Save::all()),
            first("> p.description", Save::all()),
        }
    }];
    let store = parse(FIRST_SELECTION_AS_ROOT_EARLY_EXIT, queries);

    assert_eq!(store.get(PRODUCT_SELECTOR).unwrap().count(), 1);

    let product = store.get(PRODUCT_SELECTOR).unwrap().next().unwrap();
    product.attribute(&store, "class");
    product.inner_html;
    product.text_content(&store);

    let title = product
        .get(&store, PRODUCT_TITLE_SELECTOR)
        .unwrap()
        .next()
        .unwrap();
    title.inner_html;
    title.text_content(&store);

    let rating = product
        .get(&store, PRODUCT_RATING_SELECTOR)
        .unwrap()
        .next()
        .unwrap();
    rating.inner_html;
    rating.text_content(&store);

    let description = product
        .get(&store, PRODUCT_DESCRIPTION_SELECTOR)
        .unwrap()
        .next()
        .unwrap();
    description.inner_html;
    description.text_content(&store);
}

const FIRST_CONTEXT_WITH_REQUIRED_CHILD: &str = r#"
    <div class="product"><h1>Product 1</h1></div>
    <div class="product"><span>not a title</span></div>
"#;

#[test]
fn first_context_waits_for_required_child_before_early_exit() {
    let queries = &[query! {
        first("div.product", Save::none()) => {
            first("> h1", Save::all()),
        }
    }];
    assert_eq!(queries[0].exit_at_section_end.unwrap().index(), 1);
    let store = parse(FIRST_CONTEXT_WITH_REQUIRED_CHILD, queries);

    let products = store
        .get("div.product")
        .map(|products| products.collect::<Vec<_>>())
        .unwrap_or_default();
    let titles = products
        .iter()
        .filter_map(|product| product.get(&store, "> h1"))
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(titles.len(), 1);
    assert_eq!(titles[0].text_content(&store), Some("Product 1"));
}
