//! Character references in attribute values: selectors match the decoded
//! value and the store returns it, as a DOM's `getAttribute` does (#138).

use scah::{Query, Save, parse, query};

/// How many elements `selector` matches in `html`.
fn count(html: &str, selector: &str) -> usize {
    let queries = [Query::all(selector, Save::none()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    store.get(selector).map_or(0, |elements| elements.len())
}

#[test]
fn issue_reproduction_matches_and_reads_decoded_values() {
    let html = r#"<p title="&amp;" lang="a&#x62;">x</p>"#;
    // What jsdom gives.
    assert_eq!(count(html, r#"[title="&"]"#), 1);
    assert_eq!(count(html, r#"[title="&amp;"]"#), 0);
    assert_eq!(count(html, "[lang=ab]"), 1);

    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let p = store.get("p").unwrap().next().unwrap();
    assert_eq!(p.attribute("title"), Some("&"));
    assert_eq!(p.attribute("lang"), Some("ab"));
    let attributes: Vec<_> = p
        .attributes()
        .unwrap()
        .map(|attribute| (attribute.key, attribute.value))
        .collect();
    assert_eq!(
        attributes,
        [("title", Some("&")), ("lang", Some("ab"))],
        "attributes() decodes like attribute()"
    );
}

#[test]
fn every_operator_compares_the_decoded_value() {
    let html = r#"<a data-x="fish &amp; chips&#x2D;to&#45;go">x</a>"#;
    for (selector, expected) in [
        (r#"[data-x="fish & chips-to-go"]"#, 1),
        (r#"[data-x="fish &amp; chips&#x2D;to&#45;go"]"#, 0),
        (r#"[data-x~="&"]"#, 1),
        (r#"[data-x~="chips-to-go"]"#, 1),
        (r#"[data-x~="&amp;"]"#, 0),
        ("[data-x|=fish]", 0),
        (r#"[data-x^="fish &"]"#, 1),
        (r#"[data-x^="fish &amp;"]"#, 0),
        (r#"[data-x$="-to-go"]"#, 1),
        (r#"[data-x$="&#45;go"]"#, 0),
        (r#"[data-x*="& chips-"]"#, 1),
        (r#"[data-x*="amp"]"#, 0),
        (r#"[data-x="FISH & CHIPS-TO-GO" i]"#, 1),
        (r#"[data-x="FISH &AMP; CHIPS-TO-GO" i]"#, 0),
        (r#"[data-x*="& CHIPS" i]"#, 1),
        (r#"[data-x="FISH & CHIPS-TO-GO" s]"#, 0),
    ] {
        assert_eq!(count(html, selector), expected, "{selector}");
    }

    // `|=` takes the decoded value up to its first hyphen.
    let html = r#"<p lang="&#x65;n-US">x</p>"#;
    assert_eq!(count(html, "[lang|=en]"), 1);
    assert_eq!(count(html, r#"[lang|="&#x65;n"]"#), 0);
}

#[test]
fn class_and_id_selectors_compare_decoded_values() {
    let html = concat!(
        r#"<p id="&#x61;" class="x&#32;y">1</p>"#,
        r#"<p id="b&amp;c" class="d&amp;e">2</p>"#,
        r#"<p id="plain" class="plain">3</p>"#,
    );
    // A reference to a space splits classes, as in a DOM.
    for (selector, expected) in [
        ("#a", 1),
        (".x", 1),
        (".y", 1),
        (".x.y", 1),
        ("p.x#a", 1),
        (r#"[id="b&c"]"#, 1),
        (r#"[class="d&e"]"#, 1),
        (r#"[class~="d&e"]"#, 1),
        (r#"[class="x y"]"#, 1),
        (r#"[class="x&#32;y"]"#, 0),
        ("#plain.plain", 1),
    ] {
        assert_eq!(count(html, selector), expected, "{selector}");
    }

    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let read: Vec<_> = store
        .get("p")
        .unwrap()
        .map(|p| (p.id(), p.class()))
        .collect();
    assert_eq!(
        read,
        [
            (Some("a"), Some("x y")),
            (Some("b&c"), Some("d&e")),
            (Some("plain"), Some("plain")),
        ]
    );
}

#[test]
fn duplicate_class_and_id_attributes_are_decoded_too() {
    // Only the first `class` and `id` with a value are the element's
    // fields; selectors also check later ones.
    let html = r#"<p class="first" class="&#x73;econd" id id="&#x69;">x</p>"#;
    assert_eq!(count(html, r#"[class="second"]"#), 1);
    assert_eq!(count(html, "[id=i]"), 1);
    assert_eq!(count(html, ".second"), 0);
    assert_eq!(count(html, "#i"), 1);
}

#[test]
fn legacy_references_without_semicolons_follow_the_attribute_rules() {
    let html = concat!(
        r#"<a id="kept" href="?a=1&copy=2&notit;">1</a>"#,
        r#"<a id="space" href="&copy 2">2</a>"#,
        r#"<a id="semicolon" href="&copy;=2">3</a>"#,
        r#"<a id="end" href="x&amp">4</a>"#,
    );
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let hrefs: Vec<_> = store
        .get("a")
        .unwrap()
        .map(|a| (a.id().unwrap(), a.attribute("href").unwrap()))
        .collect();
    assert_eq!(
        hrefs,
        [
            ("kept", "?a=1&copy=2&notit;"),
            ("space", "© 2"),
            ("semicolon", "©=2"),
            ("end", "x&"),
        ]
    );
    assert_eq!(count(html, r#"[href="?a=1&copy=2&notit;"]"#), 1);
    assert_eq!(count(html, r#"[href="© 2"]"#), 1);
    assert_eq!(count(html, r#"[href="©=2"]"#), 1);
}

#[test]
fn numeric_references_follow_the_text_rules() {
    let html = r#"<p title="&#x62;&#98;" lang="&#0;" dir="&#x80;" data-surrogate="&#xD800;">x</p>"#;
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let p = store.get("p").unwrap().next().unwrap();
    assert_eq!(p.attribute("title"), Some("bb"));
    assert_eq!(p.attribute("lang"), Some("\u{FFFD}"));
    assert_eq!(p.attribute("dir"), Some("€"));
    assert_eq!(p.attribute("data-surrogate"), Some("\u{FFFD}"));
    assert_eq!(count(html, "[dir=€]"), 1);
    assert_eq!(count(html, "[lang=\"\u{FFFD}\"]"), 1);
}

#[test]
fn values_without_ampersands_stay_slices_of_the_html() {
    let html = r#"<a id="i" class="c" href="/plain" title="&amp;">x</a>"#;
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let a = store.get("a").unwrap().next().unwrap();
    let in_html = |value: &str| html.as_bytes().as_ptr_range().contains(&value.as_ptr());
    for (value, quoted) in [
        (a.id(), r#""i""#),
        (a.class(), r#""c""#),
        (a.attribute("href"), r#""/plain""#),
    ] {
        // The very bytes between the quotes, not an equal copy.
        let start = html.find(quoted).unwrap() + 1;
        let expected = &html[start..start + quoted.len() - 2];
        assert_eq!(value, Some(expected));
        assert_eq!(
            value.unwrap().as_ptr(),
            expected.as_ptr(),
            "{quoted} was copied"
        );
    }
    assert_eq!(a.attribute("title"), Some("&"));
    assert!(
        !in_html(a.attribute("title").unwrap()),
        "decoded in the store"
    );
}

#[test]
fn values_are_decoded_when_first_read() {
    let html = r#"<a href="?a=1&amp;b=2" title="&lt;x&gt;">x</a>"#;
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let unread = store.heap_usage().attributes;
    let a = store.get("a").unwrap().next().unwrap();
    assert_eq!(a.attribute("href"), Some("?a=1&b=2"));
    assert!(store.heap_usage().attributes > unread, "decoded on read");
    // Every value with `&` was decoded by the first read.
    let decoded = store.heap_usage().attributes;
    assert_eq!(a.attribute("title"), Some("<x>"));
    assert_eq!(store.heap_usage().attributes, decoded);
}

#[test]
fn decoded_values_match_inside_logical_and_structural_selectors() {
    let html = concat!(
        "<ul>",
        r#"<li title="a&amp;b">1</li>"#,
        r#"<li title="c">2</li>"#,
        r#"<li title="a&amp;b">3</li>"#,
        "</ul>",
    );
    assert_eq!(count(html, r#"li:not([title="a&b"])"#), 1);
    assert_eq!(count(html, r#"li:is([title="a&b"], [title=x])"#), 2);
    assert_eq!(count(html, r#"li:nth-child(2 of [title="a&b"])"#), 1);
    assert_eq!(count(html, r#"li[title="a&b"] + li"#), 1);
    assert_eq!(count(html, r#"li[title="a&b"] ~ li[title="a&b"]"#), 1);
}

#[test]
fn compile_time_queries_compare_decoded_values() {
    let queries = [query! { all(r#"a[href="?a=1&b=2"]"#, Save::all()) }];
    let html = r#"<a href="?a=1&amp;b=2">x</a><a href="?a=1&b=2">y</a>"#;
    let store = parse(html, &queries).unwrap();
    let hrefs: Vec<_> = store
        .query(0)
        .unwrap()
        .map(|a| a.attribute("href").unwrap())
        .collect();
    assert_eq!(hrefs, ["?a=1&b=2", "?a=1&b=2"]);
}

#[test]
fn first_and_nested_queries_save_decoded_values() {
    let html = r#"<div data-k="&lt;1&gt;"><a href="/x?a&amp;b">x</a></div><div data-k="2"></div>"#;
    let queries = [Query::first(r#"div[data-k="<1>"]"#, Save::all())
        .unwrap()
        .then(|div| Ok([div.all("a", Save::all())?]))
        .unwrap()
        .build()];
    let store = parse(html, &queries).unwrap();
    let div = store.query(0).unwrap().next().unwrap();
    assert_eq!(div.attribute("data-k"), Some("<1>"));
    let a = div.get("a").unwrap().next().unwrap();
    assert_eq!(a.attribute("href"), Some("/x?a&b"));
}
