use super::helpers::{elements, inner_htmls, parse_all, parse_with_saves, texts};
use scah::{Query, Save, parse};

#[test]
fn empty_or_whitespace_only_saved_text_does_not_panic() {
    let store = parse_with_saves(
        "<div></div><p>   </p>",
        &[
            ("div", Save::all()),
            ("p", Save::only_text_content()),
            ("span", Save::only_inner_html()),
        ],
    );

    assert_eq!(elements(&store, "div").len(), 1);
    assert_eq!(texts(&store, "div"), vec![None]);
    assert_eq!(inner_htmls(&store, "div"), vec![Some("")]);
    assert_eq!(texts(&store, "p"), vec![None]);
}

#[test]
fn id_and_class_are_available_to_attribute_presence_selectors() {
    let html = r#"<p id="x" class="foo bar" data-id="x">text</p>"#;
    let store = parse_all(
        html,
        &[
            "#x",
            ".foo",
            "p[id]",
            r#"p[id="x"]"#,
            "p[class]",
            r#"p[class="foo bar"]"#,
            r#"p[data-id="x"]"#,
        ],
    );

    for selector in [
        "#x",
        ".foo",
        "p[id]",
        r#"p[id="x"]"#,
        "p[class]",
        r#"p[class="foo bar"]"#,
        r#"p[data-id="x"]"#,
    ] {
        assert_eq!(elements(&store, selector).len(), 1, "{selector}");
    }
}

#[test]
fn comments_are_scanned_until_comment_close() {
    let html = "<!-- a > <a>fake</a> --><a>real</a>";
    let store = parse_all(html, &["a"]);

    assert_eq!(texts(&store, "a"), vec![Some("real")]);
}

#[test]
fn raw_text_elements_do_not_emit_false_selector_matches() {
    let html =
        r#"<style>.x:before{content:"<a>"}</style><textarea><a>fake</a></textarea><a>real</a>"#;
    let store = parse_all(html, &["a", "style", "textarea"]);

    assert_eq!(texts(&store, "a"), vec![Some("real")]);
    assert_eq!(elements(&store, "style").len(), 1);
    assert_eq!(elements(&store, "textarea").len(), 1);
}

#[test]
fn nested_descendant_queries_do_not_duplicate_matches() {
    let html = "\
        <main><section>\
            <a href='h2'>Hello2</a>\
            <div><a href='w2'>World2</a><div><a href='w3'>World3</a></div></div>\
        </section></main>\
    ";
    let query = Query::all("main > section", Save::all())
        .unwrap()
        .then(|section| Ok([section.all("div a", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries);
    let section = store.get("main > section").unwrap().next().unwrap();
    let links = section.get(&store, "div a").unwrap().collect::<Vec<_>>();

    assert_eq!(links.len(), 2);
    assert_eq!(
        links
            .iter()
            .map(|link| (link.attribute(&store, "href"), link.text_content(&store)))
            .collect::<Vec<_>>(),
        vec![(Some("w2"), Some("World2")), (Some("w3"), Some("World3"))]
    );
}

#[test]
fn nested_first_query_returns_only_first_match_per_parent() {
    let html = "<div><a>1</a><a>2</a></div><div><a>3</a><a>4</a></div>";
    let query = Query::all("div", Save::none())
        .unwrap()
        .then(|parent| Ok([parent.first("> a", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries);
    let divs = store.get("div").unwrap().collect::<Vec<_>>();

    assert_eq!(divs.len(), 2);
    assert_eq!(
        divs.iter()
            .map(|div| {
                div.get(&store, "> a")
                    .unwrap()
                    .map(|a| a.text_content(&store).unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec!["1"], vec!["3"]]
    );
}
