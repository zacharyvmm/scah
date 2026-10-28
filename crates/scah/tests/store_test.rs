//! How parses fill the store: which columns and buffers they reserve, and
//! how results are looked up. The store's own tests are in scah-store; these
//! need a parse, and read the store's internals through its hidden accessors.

use scah::{Query, Reader, Save, Store, XHtmlParser, parse};

fn tracks_text(store: &Store) -> bool {
    !store.columns().text.is_empty()
}

fn tracks_raw_text(store: &Store) -> bool {
    !store.columns().raw_text.is_empty()
}

#[test]
fn parse_inner_html_only_skips_text_range_sidecar() {
    let html = "<p>one</p><p>two</p>";
    let query = Query::all("p", Save::only_inner_html()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    assert!(!tracks_text(&store) && !tracks_raw_text(&store));
    assert_eq!(store.len(), 2);
}

#[test]
fn parse_text_modes_allocate_independent_range_vectors() {
    let html = "<p class=\"x\">A</p><p>B</p>";
    let queries = [
        Query::all("p", Save::only_text()).unwrap().build(),
        Query::all("p.x", Save::only_raw_text()).unwrap().build(),
    ];
    let store = parse(html, &queries).unwrap();
    assert!(tracks_text(&store) || tracks_raw_text(&store));
    assert!(tracks_raw_text(&store));
    assert!(tracks_text(&store));
    assert_eq!(store.len(), 3);
}

#[test]
fn query_aware_capacity_keeps_unmatched_text_storage_lazy() {
    let html = "<main><div data-a='1'>outside</div><p>more</p></main>";
    let query = Query::all(".missing", Save::only_text()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.text_tapes().raw_text.capacity(), 0);
    assert_eq!(store.text_tapes().text.capacity(), 0);
    assert!(!tracks_text(&store) && !tracks_raw_text(&store));
}

#[test]
fn one_text_mode_does_not_allocate_ranges_for_the_other() {
    let html = "<p>A</p><p>B</p>";
    let query = Query::all("p", Save::only_text()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert!(!tracks_raw_text(&store));
    assert!(tracks_text(&store));
}

#[test]
fn positional_lookups_match_selector_lookups() {
    let queries = [
        Query::all("ul", Save::none())
            .unwrap()
            .then(|list| {
                Ok([
                    list.all("> li", Save::none())?,
                    list.first("a", Save::none())?,
                ])
            })
            .unwrap()
            .build(),
        Query::all("p", Save::none()).unwrap().build(),
    ];
    let store = parse(
        "<ul><li><a>1</a></li><li>2</li></ul><p></p><ul></ul>",
        &queries,
    )
    .unwrap();

    let lists: Vec<_> = store.query_results(0).unwrap().to_vec();
    assert_eq!(lists.len(), 2);
    assert_eq!(store.query_selector(0), Some("ul"));
    assert_eq!(store.query_selector(1), Some("p"));
    assert_eq!(store.query_selector(2), None);
    assert_eq!(store.query_results(1), store.results("p"));
    assert_eq!(store.query_results(2), None);

    let [full, empty] = [lists[0], lists[1]];
    assert_eq!(store.nested_results(full, 0).map(<[_]>::len), Some(2));
    assert_eq!(
        store.nested_results(full, 0),
        store.child_results(full, "> li")
    );
    assert_eq!(
        store.nested_results(full, 1),
        store.child_results(full, "a")
    );
    assert_eq!(store.nested_selector(full, 1), Some("a"));
    assert_eq!(store.nested_results(full, 2), None);
    assert_eq!(store.nested_selector(full, 2), None);
    assert_eq!(store.nested_results(empty, 0), None);

    // Leaf rows have no nested sections.
    let item = store.nested_results(full, 0).unwrap()[0];
    assert_eq!(store.nested_results(item, 0), None);
    assert_eq!(store.child_results(item, "a"), None);
}

#[test]
fn a_dropped_store_lends_its_buffers_to_the_next_parse() {
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    let html = "<p class='a'>one</p><p>two</p>".repeat(50);
    let (rows, text) = {
        let store = parse(&html, &queries).unwrap();
        (
            store.columns().rows.as_ptr(),
            store.text_tapes().text.as_bytes().as_ptr(),
        )
    };
    let store = parse(&html, &queries).unwrap();
    assert_eq!(store.columns().rows.as_ptr(), rows);
    assert_eq!(store.text_tapes().text.as_bytes().as_ptr(), text);
}

#[test]
fn reservations_do_not_affect_equality() {
    let html = "<p class=a>one</p><p>two</p>";
    let queries = [Query::all("p", Save::all()).unwrap().build()];
    fn parse<'html, 'query: 'html>(
        mut parser: XHtmlParser<'html, 'query>,
        html: &'html str,
    ) -> Store<'html, 'query> {
        let mut reader = Reader::new(html);
        while parser.next(&mut reader) {}
        parser.matches()
    }
    let plain = parse(XHtmlParser::new(&queries), html);
    let reserved = parse(XHtmlParser::with_capacity(&queries, 1 << 16), html);
    assert_ne!(
        plain.columns().reserved_rows,
        reserved.columns().reserved_rows
    );
    assert_eq!(plain, reserved);
}

#[test]
fn optional_columns_do_not_copy_reused_row_capacity() {
    let names = [Query::all("p", Save::name_only()).unwrap().build()];
    drop(parse(&"<p>x</p>".repeat(5_000), &names).unwrap());

    // An early-exit parse reserves no rows, so its optional columns grow
    // with the rows it saves, not with the rows the last parse kept.
    let first = [Query::first("p", Save::all()).unwrap().build()];
    let store = parse("<p>one</p><p>two</p>", &first).unwrap();
    assert!(store.columns().rows.capacity() >= 5_000);
    assert!(store.columns().attribute_keys.capacity() < 100);
    assert!(store.columns().inner_html.capacity() < 100);
    assert!(store.columns().text.capacity() < 100);
}

#[test]
fn class_and_id_are_the_first_with_a_value_and_the_rest_keep_source_order() {
    let queries = [Query::all("a", Save::all()).unwrap().build()];
    let html = r#"<a class href=h id=x class=c data-k class=d id><b></b></a>"#;
    let store = parse(html, &queries).unwrap();
    let a = store.get("a").unwrap().next().unwrap();

    assert_eq!((a.class(), a.id()), (Some("c"), Some("x")));
    let others: Vec<_> = a
        .attributes()
        .unwrap()
        .map(|attribute| (attribute.key, attribute.value))
        .collect();
    assert_eq!(
        others,
        [
            ("class", None),
            ("href", Some("h")),
            ("data-k", None),
            ("class", Some("d")),
            ("id", None),
        ]
    );
    // `attribute` searches the same attributes as `attributes`.
    assert_eq!(a.attribute("href"), Some("h"));
    assert_eq!(a.attribute("CLASS"), None);
    assert_eq!(a.attribute("data-k"), None);
    assert_eq!(store.attribute_count(), 7);

    // Without `class` or `id`, the attributes start with the others.
    let html = r#"<a ID data-x=1>t</a>"#;
    let store = parse(html, &queries).unwrap();
    let a = store.get("a").unwrap().next().unwrap();
    assert_eq!((a.class(), a.id()), (None, None));
    assert_eq!(a.attributes().unwrap().len(), 2);
    assert_eq!(a.attribute("data-x"), Some("1"));
}

#[test]
fn reused_buffers_start_empty() {
    let text_queries = [Query::all("p", Save::all()).unwrap().build()];
    drop(parse("<p id=x class=y z=1>first</p><p>second</p>", &text_queries).unwrap());

    let html = String::from("<div><p>third</p></div>");
    let name_queries = [Query::all("p", Save::name_only()).unwrap().build()];
    let store = parse(&html, &name_queries).unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.attribute_count(), 0);
    let p = store.get("p").unwrap().next().unwrap();
    assert_eq!((p.name(), p.id(), p.class()), ("p", None, None));
    assert_eq!(
        (p.attributes().is_none(), p.text(), p.inner_html()),
        (true, None, None)
    );
    drop(store);

    let store = parse(&html, &text_queries).unwrap();
    let p = store.get("p").unwrap().next().unwrap();
    assert_eq!(p.text(), Some("third"));
    assert_eq!(store.text_tapes().text.as_bytes(), b"third");
}

#[test]
fn queries_with_the_same_selector_keep_separate_results() {
    let queries = [
        Query::all("a", Save::none()).unwrap().build(),
        Query::all("a", Save::all()).unwrap().build(),
    ];
    let store = parse("<a href=x>1</a><a>2</a>", &queries).unwrap();

    assert_eq!(store.query(0).unwrap().count(), 2);
    assert_eq!(store.query(1).unwrap().count(), 2);
    assert!(store.query(0).unwrap().all(|a| a.inner_html().is_none()));
    assert!(store.query(1).unwrap().all(|a| a.inner_html().is_some()));
    // `get` resolves a shared selector to the first query.
    assert!(store.get("a").unwrap().all(|a| a.inner_html().is_none()));
    assert!(store.query(2).is_none());
}
