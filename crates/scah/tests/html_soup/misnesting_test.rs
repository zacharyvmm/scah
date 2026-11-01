use super::helpers::{attr, elements, inner_htmls, parse_all, texts};

#[test]
fn stray_end_tag_is_ignored_for_selection_but_preserved_in_inner_html() {
    let html = "<div><span>Hello</bogus></span></div>";
    let store = parse_all(html, &["div span"]);

    assert_eq!(elements(&store, "div span").len(), 1);
    assert_eq!(texts(&store, "div span"), vec![Some("Hello")]);
    assert_eq!(inner_htmls(&store, "div span"), vec![Some("Hello</bogus>")]);
}

#[test]
fn closing_ancestor_before_descendant_bubbles_recovery_without_duplicate_matches() {
    let html = "<div><span>Hello</div>";
    let store = parse_all(html, &["div", "span", "div span"]);

    assert_eq!(elements(&store, "div").len(), 1);
    assert_eq!(elements(&store, "span").len(), 1);
    assert_eq!(elements(&store, "div span").len(), 1);
    assert_eq!(texts(&store, "span"), vec![Some("Hello")]);
    assert_eq!(inner_htmls(&store, "span"), vec![Some("Hello")]);
    assert_eq!(inner_htmls(&store, "div"), vec![Some("<span>Hello")]);
    assert_eq!(texts(&store, "div"), vec![Some("Hello")]);
}

#[test]
fn extra_end_tag_after_valid_close_does_not_create_extra_matches() {
    let html = "<section><a href='x'>Link</a></a></section>";
    let store = parse_all(html, &["a", "section a"]);

    assert_eq!(elements(&store, "a").len(), 1);
    assert_eq!(elements(&store, "section a").len(), 1);
    assert_eq!(attr(&store, "a", "href"), vec![Some("x")]);
    assert_eq!(texts(&store, "a"), vec![Some("Link")]);
}

#[test]
fn stray_end_tags_under_deep_nesting_are_ignored() {
    let depth = 3_000;
    let html = format!(
        "<main>{}<x-item>a{}{}</X-ITEM>b{}<p>c</p></main>",
        "<div>".repeat(depth),
        "</span>".repeat(depth),
        "</x-other>".repeat(depth),
        "</div>".repeat(depth)
    );
    let store = parse_all(&html, &["x-item", "div > x-item", "main > div", "main > p"]);

    assert_eq!(texts(&store, "x-item"), vec![Some("a")]);
    assert_eq!(elements(&store, "div > x-item").len(), 1);
    assert_eq!(elements(&store, "main > div").len(), 1);
    assert_eq!(texts(&store, "main > p"), vec![Some("c")]);
}

#[test]
fn end_tags_below_a_barrier_are_ignored_under_deep_nesting() {
    let depth = 3_000;
    let html = format!(
        "<span><x-item><table>{}{}{}{}</table></x-item></span><b>u</b>",
        "<div>".repeat(depth),
        "</span>".repeat(depth),
        "</x-item>".repeat(depth),
        "</div>".repeat(depth)
    );
    let store = parse_all(&html, &["span > x-item > table", "table div", "span b"]);

    assert_eq!(elements(&store, "span > x-item > table").len(), 1);
    assert_eq!(elements(&store, "table div").len(), depth);
    assert!(elements(&store, "span b").is_empty());
}
