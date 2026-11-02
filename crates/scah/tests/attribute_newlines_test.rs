//! CR and NUL in attribute values: a browser's tokenizer reads CRLF and a
//! lone CR written in the HTML as LF, and a NUL as U+FFFD, so selectors
//! match and the store returns the value that way, as a DOM's
//! `getAttribute` does (#153).

use scah::{Query, Save, parse};

/// How many elements `selector` matches in `html`.
fn count(html: &str, selector: &str) -> usize {
    let queries = [Query::all(selector, Save::none()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    store.get(selector).map_or(0, |elements| elements.len())
}

#[test]
fn issue_reproduction_reads_and_matches_normalized_values() {
    let html = "<p title=\"a\r\nb\" lang=\"x\0y\" dir=\"c\rd\">t</p>";
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let p = store.get("p").unwrap().next().unwrap();
    // What a browser's `getAttribute` gives.
    assert_eq!(p.attribute("title"), Some("a\nb"));
    assert_eq!(p.attribute("lang"), Some("x\u{FFFD}y"));
    assert_eq!(p.attribute("dir"), Some("c\nd"));
    let attributes: Vec<_> = p
        .attributes()
        .unwrap()
        .map(|attribute| (attribute.key, attribute.value))
        .collect();
    assert_eq!(
        attributes,
        [
            ("title", Some("a\nb")),
            ("lang", Some("x\u{FFFD}y")),
            ("dir", Some("c\nd")),
        ],
        "attributes() decodes like attribute()"
    );

    assert_eq!(count(html, "[lang=\"x\u{FFFD}y\"]"), 1);
    // scah accepts newlines in selector strings, which CSS doesn't, and
    // compares them with the normalized value.
    assert_eq!(count(html, "[title=\"a\nb\"]"), 1);
    assert_eq!(count(html, "[title=\"a\r\nb\"]"), 0);
    assert_eq!(count(html, "[dir=\"c\nd\"]"), 1);
    assert_eq!(count(html, "[dir=\"c\rd\"]"), 0);
}

#[test]
fn every_operator_compares_the_normalized_value() {
    let html = "<a data-x=\"one\r\ntwo\rthree\0four\">x</a>";
    for (selector, expected) in [
        ("[data-x=\"one\ntwo\nthree\u{FFFD}four\"]", 1),
        ("[data-x~=two]", 1),
        ("[data-x~=\"three\u{FFFD}four\"]", 1),
        ("[data-x~=three]", 0),
        ("[data-x|=\"one\ntwo\nthree\u{FFFD}four\"]", 1),
        ("[data-x^=\"one\ntwo\"]", 1),
        ("[data-x^=\"one\r\"]", 0),
        ("[data-x$=\"\u{FFFD}four\"]", 1),
        ("[data-x*=\"two\nthree\"]", 1),
        ("[data-x*=\"\r\"]", 0),
        ("[data-x=\"ONE\nTWO\nTHREE\u{FFFD}FOUR\" i]", 1),
        ("[data-x=\"ONE\nTWO\nTHREE\u{FFFD}FOUR\" s]", 0),
    ] {
        assert_eq!(count(html, selector), expected, "{selector:?}");
    }
}

#[test]
fn class_and_id_selectors_compare_normalized_values() {
    let html = concat!(
        "<p id=\"i\0\" class=\"a\r\nb\rc\">1</p>",
        "<p id=\"j\" class=\"x\0 y\">2</p>",
    );
    // CR and LF both separate classes, before and after normalizing.
    for (selector, expected) in [
        (".a", 1),
        (".b", 1),
        (".c", 1),
        (".a.b.c", 1),
        ("[class~=b]", 1),
        ("[class=\"a\nb\nc\"]", 1),
        ("[class=\"a\r\nb\rc\"]", 0),
        (".x", 0),
        (".y", 1),
        ("[class~=\"x\u{FFFD}\"]", 1),
        ("#i", 0),
        ("[id=\"i\u{FFFD}\"]", 1),
        ("#j.y", 1),
    ] {
        assert_eq!(count(html, selector), expected, "{selector:?}");
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
            (Some("i\u{FFFD}"), Some("a\nb\nc")),
            (Some("j"), Some("x\u{FFFD} y")),
        ]
    );
}

#[test]
fn only_source_crs_and_nuls_change_not_referenced_ones() {
    // `&#13;` is a CR from a reference, which browsers keep; only a CR
    // written in the HTML is normalized. `&#0;` is U+FFFD by the numeric
    // reference rules.
    let html = concat!(
        "<p id=\"mixed\" title=\"&amp;\r\n&#13;&#10;\r\0&#0;&lt;\">1</p>",
        "<p id=\"referenced\" title=\"x&#13;y\">2</p>",
        "<p id=\"written\" title=\"x\ry\">3</p>",
        "<p id=\"crlf\" title=\"x&#x0D;\ny\">4</p>",
        "<p id=\"split\" title=\"x\r&amp;\ny\">5</p>",
    );
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let titles: Vec<_> = store
        .get("p")
        .unwrap()
        .map(|p| (p.id().unwrap(), p.attribute("title").unwrap()))
        .collect();
    assert_eq!(
        titles,
        [
            ("mixed", "&\n\r\n\n\u{FFFD}\u{FFFD}<"),
            ("referenced", "x\ry"),
            ("written", "x\ny"),
            ("crlf", "x\r\ny"),
            ("split", "x\n&\ny"),
        ]
    );
    assert_eq!(count(html, "[title=\"x\ry\"]"), 1);
    assert_eq!(count(html, "[title=\"x\ny\"]"), 1);
    assert_eq!(count(html, "[title=\"x\r\ny\"]"), 1);
    assert_eq!(count(html, "[title=\"&\n\r\n\n\u{FFFD}\u{FFFD}<\"]"), 1);
}

#[test]
fn single_quoted_and_unquoted_values_are_normalized_too() {
    let html = "<p title='a\r\nb' lang=x\0y>t</p>";
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let p = store.get("p").unwrap().next().unwrap();
    assert_eq!(p.attribute("title"), Some("a\nb"));
    assert_eq!(p.attribute("lang"), Some("x\u{FFFD}y"));
    assert_eq!(count(html, "[lang=\"x\u{FFFD}y\"]"), 1);
}

#[test]
fn values_without_ampersands_crs_or_nuls_stay_slices_of_the_html() {
    let html = "<a id=\"i\" class=\"c\nd\" href=\"/plain\ttab\" title=\"t\r\n\" lang=\"l\0\">x</a>";
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let a = store.get("a").unwrap().next().unwrap();
    for (value, quoted) in [
        (a.id(), "\"i\""),
        (a.class(), "\"c\nd\""),
        (a.attribute("href"), "\"/plain\ttab\""),
    ] {
        // The very bytes between the quotes, not an equal copy.
        let start = html.find(quoted).unwrap() + 1;
        let expected = &html[start..start + quoted.len() - 2];
        assert_eq!(value, Some(expected));
        assert_eq!(
            value.unwrap().as_ptr(),
            expected.as_ptr(),
            "{quoted:?} was copied"
        );
    }
    let in_html = |value: &str| html.as_bytes().as_ptr_range().contains(&value.as_ptr());
    for (key, expected) in [("title", "t\n"), ("lang", "l\u{FFFD}")] {
        let value = a.attribute(key).unwrap();
        assert_eq!(value, expected);
        assert!(!in_html(value), "{key} decoded in the store");
    }
}

#[test]
fn values_are_normalized_when_first_read() {
    let html = "<a href=\"x\r\ny\" title=\"z\0\">x</a>";
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let store = parse(html, &queries).unwrap();
    let unread = store.heap_usage().attributes;
    let a = store.get("a").unwrap().next().unwrap();
    assert_eq!(a.attribute("href"), Some("x\ny"));
    assert!(store.heap_usage().attributes > unread, "decoded on read");
    // Every value that needs it was decoded by the first read.
    let decoded = store.heap_usage().attributes;
    assert_eq!(a.attribute("title"), Some("z\u{FFFD}"));
    assert_eq!(store.heap_usage().attributes, decoded);
}
