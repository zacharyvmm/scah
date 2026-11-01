use super::helpers::{elements, inner_htmls, parse_all, texts};

#[test]
fn implied_p_close_splits_content_correctly() {
    let html = "<div><p>Hello<div>World</div></div>";
    let store = parse_all(html, &["p", "div", "div > p", "div > div"]);

    assert_eq!(elements(&store, "p").len(), 1);
    assert_eq!(texts(&store, "p"), vec![Some("Hello")]);
    assert_eq!(inner_htmls(&store, "p"), vec![Some("Hello")]);
    assert_eq!(
        texts(&store, "div"),
        vec![Some("Hello\nWorld"), Some("World")]
    );
    assert_eq!(elements(&store, "div > p").len(), 1);
    assert_eq!(elements(&store, "div > div").len(), 1);
}

#[test]
fn implied_li_close_creates_distinct_siblings() {
    let html = "<ul><li>One<li>Two<li>Three</ul>";
    let store = parse_all(html, &["li", "ul > li"]);

    assert_eq!(elements(&store, "li").len(), 3);
    assert_eq!(
        texts(&store, "li"),
        vec![Some("One"), Some("Two"), Some("Three")]
    );
    assert_eq!(
        inner_htmls(&store, "li"),
        vec![Some("One"), Some("Two"), Some("Three")]
    );
    assert_eq!(elements(&store, "ul > li").len(), 3);
}

#[test]
fn implied_dt_dd_close_creates_expected_pairs() {
    let html = "<dl><dt>A<dd>B<dt>C<dd>D</dl>";
    let store = parse_all(html, &["dt", "dd", "dl > dt", "dl > dd"]);

    assert_eq!(texts(&store, "dt"), vec![Some("A"), Some("C")]);
    assert_eq!(texts(&store, "dd"), vec![Some("B"), Some("D")]);
    assert_eq!(elements(&store, "dl > dt").len(), 2);
    assert_eq!(elements(&store, "dl > dd").len(), 2);
}

#[test]
fn implied_option_close_splits_options() {
    let html = "<select><option>One<option>Two<option>Three</select>";
    let store = parse_all(html, &["option", "select > option"]);

    assert_eq!(elements(&store, "option").len(), 3);
    assert_eq!(
        texts(&store, "option"),
        vec![Some("One"), Some("Two"), Some("Three")]
    );
    assert_eq!(
        inner_htmls(&store, "option"),
        vec![Some("One"), Some("Two"), Some("Three")]
    );
    assert_eq!(elements(&store, "select > option").len(), 3);
}

#[test]
fn implied_closes_stay_correct_under_deep_nesting() {
    let depth = 3_000;
    let html = format!(
        "<ul><li>a<p>x{}<p>y</p>{}<li>b</ul><p>z<div>w</div>",
        "<section>".repeat(depth),
        "</section>".repeat(depth)
    );
    let store = parse_all(&html, &["p", "ul > li", "li > p", "section > p", "p div"]);

    assert_eq!(texts(&store, "p"), vec![Some("x"), Some("y"), Some("z")]);
    assert_eq!(elements(&store, "ul > li").len(), 2);
    assert_eq!(elements(&store, "li > p").len(), 1);
    assert_eq!(elements(&store, "section > p").len(), 1);
    assert!(elements(&store, "p div").is_empty());
}

#[test]
fn deep_nesting_above_a_barrier_keeps_the_p_open() {
    let depth = 3_000;
    let html = format!(
        "<p>x<object>{}{}</object><div>y</div>",
        "<div>".repeat(depth),
        "</div>".repeat(depth)
    );
    let store = parse_all(&html, &["p", "p > object > div", "p div"]);

    assert_eq!(texts(&store, "p"), vec![Some("x")]);
    assert_eq!(elements(&store, "p > object > div").len(), 1);
    assert_eq!(elements(&store, "p div").len(), depth);
}
