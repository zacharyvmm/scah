use std::ops::Deref;

#[cfg(debug_assertions)]
use scah::debug;
use scah::{Attribute, Query, QuerySpec, Save, Store, parse, query};
const HTML: &str = r#"
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
#[cfg(debug_assertions)]
fn trace_records_open_and_save_events() {
    let html = "<main><section><a href='/x'>x</a></section></main>";
    let query = Query::all("main section a", Save::all()).unwrap().build();
    let queries = [query];

    let store = parse(html, &queries).unwrap();

    assert!(!store.trace.is_empty());
    assert!(
        store
            .trace
            .events()
            .iter()
            .any(|event| { matches!(event, debug::TraceEvent::OpenTag { tag: "main", .. }) })
    );
    assert!(
        store
            .trace
            .events()
            .iter()
            .any(|event| { matches!(event, debug::TraceEvent::ElementSaved { element: "a", .. }) })
    );
}

#[test]
#[cfg(debug_assertions)]
fn trace_records_implied_li_close() {
    let html = "<ul><li>One<li>Two</ul>";
    let query = Query::all("li", Save::all()).unwrap().build();
    let queries = [query];

    let store = parse(html, &queries).unwrap();

    assert!(store.trace.events().iter().any(|event| {
        matches!(
            event,
            debug::TraceEvent::ImpliedClose {
                tag: "li",
                reason: debug::ImpliedCloseReason::OpenTagRule,
                ..
            }
        )
    }));
}

#[test]
#[cfg(debug_assertions)]
fn trace_records_first_query_early_exit() {
    let html = "<div><a>one</a><a>two</a></div>";
    let query = Query::first("a", Save::all()).unwrap().build();
    let queries = [query];

    let store = parse(html, &queries).unwrap();

    assert!(
        store
            .trace
            .events()
            .iter()
            .any(|event| matches!(event, debug::TraceEvent::EarlyExit { .. }))
    );
}

#[test]
#[cfg(debug_assertions)]
fn trace_records_transition_rejections() {
    let html = "<main><span>no</span><a>yes</a></main>";
    let query = Query::all("main > a", Save::all()).unwrap().build();
    let queries = [query];

    let store = parse(html, &queries).unwrap();

    assert!(store.trace.events().iter().any(|event| {
        matches!(
            event,
            debug::TraceEvent::TransitionRejected {
                element: "span",
                reason: debug::TransitionRejectReason::PredicateFailed,
                ..
            }
        )
    }));
}

#[test]
fn test_html_page() {
    let selection_tree = Query::all("main > section#id", Save::all()).unwrap();

    let queries = &[selection_tree.build()];
    let store = parse(HTML, queries).unwrap();
    let list = store.get("main > section#id").unwrap().collect::<Vec<_>>();

    assert_eq!(list.len(), 2);

    let last = list.last().unwrap();

    assert!(last.inner_html.is_some());
    assert_eq!(
        last.inner_html.unwrap().trim(),
        r#"<a href="wrong-main">Not selected (main has no red-background class)</a>"#
    );

    assert!(last.text(&store).is_some());
    assert_eq!(
        last.text(&store).unwrap(),
        r#"Not selected (main has no red-background class)"#
    );

    let first = list.first().unwrap();
    assert_eq!(
        first.inner_html.unwrap().trim(),
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
        first.text(&store).unwrap(),
        "Link 1 Link 2 Link 3\nNot selected (nested in div)\nNo link here"
    );
}

#[test]
fn test_html_page_all_anchor_tag_selection() {
    let queries = &[Query::all("a", Save::all()).unwrap().build()];
    let store = parse(HTML, queries).unwrap();
    println!("Store: {:#?}", store);

    let list = store.get("a").unwrap().collect::<Vec<_>>();

    assert_eq!(list.len(), 7);
    println!("List: {:#?}", list);
}

#[test]
fn test_html_page_first_anchor_tag_selection() {
    let queries = &[Query::first("a", Save::all()).unwrap().build()];
    let store = parse(HTML, queries).unwrap();
    let mut children = store.get("a").unwrap();

    let a = children.next().unwrap();
    assert_eq!(
        store.attributes.deref().clone(),
        vec![Attribute {
            key: "href",
            value: Some("link1")
        }]
    );
    assert_eq!(a.name, "a");
    assert_eq!(
        a.attributes(&store).unwrap(),
        &[Attribute {
            key: "href",
            value: Some("link1")
        }]
    );
    assert_eq!(a.attribute(&store, "href"), Some("link1"));
    assert_eq!(a.text(&store).unwrap(), "Link 1");
}

#[test]
fn test_html_page_all_anchor_tag_starting_with_link_selection() {
    let queries = &[Query::all("a[href^=link]", Save::all()).unwrap().build()];
    let store = parse(HTML, queries).unwrap();
    let list = store.get("a[href^=link]").unwrap();

    assert_eq!(list.count(), 3);
}

#[test]
fn test_html_page_children_valid_anchor_tags_in_main() {
    let queries = &[Query::all("main > section > a[href]", Save::all())
        .unwrap()
        .build()];

    let store = parse(HTML, queries).unwrap();
    let list = store.get("main > section > a[href]").unwrap();

    assert_eq!(list.count(), 5);
}

#[test]
fn test_html_page_single_main() {
    let queries = &[Query::all("main.red-background > section#id", Save::all())
        .unwrap()
        .build()];
    let store = parse(HTML, queries).unwrap();
    let list = store.get("main.red-background > section#id").unwrap();

    assert_eq!(list.count(), 1);
}

#[test]
fn test_html_multi_selection() {
    let query = Query::all("main > section", Save::all())
        .unwrap()
        .then(|section| {
            Ok([
                // BUG: first selection not working because their is no locking mechanism
                //section.first("> a[href]", Save::all()),
                section.all("> a[href]", Save::all())?,
                section.all("div a", Save::all())?,
                // BUG: If their are 2 identical sub-queries their should be an error.
                //section.all("> a[href]", Save::all()),
            ])
        })
        .unwrap()
        .build();

    let q = &[query];
    let store = parse(HTML, q).unwrap();
    let list = store.get("main > section").unwrap();

    println!("List: {:#?}", list.collect::<Vec<_>>());
}

#[test]
fn test_macro_static_query() {
    let static_query = query! {
        all("main > section", Save::all()) => {
            all("> a[href]", Save::all()),
            first("span", Save::only_text()),
        }
    };
    let runtime_query = Query::all("main > section", Save::all())
        .unwrap()
        .then(|ctx| {
            Ok([
                ctx.all("> a[href]", Save::all())?,
                ctx.first("span", Save::only_text())?,
            ])
        })
        .unwrap()
        .build();

    let static_queries = [static_query];
    let runtime_queries = [runtime_query];
    let static_store = parse(HTML, &static_queries).unwrap();
    let runtime_store = parse(HTML, &runtime_queries).unwrap();
    let count = |store: &scah::Store<'_, '_>, selector| {
        store.get(selector).map(|items| items.count()).unwrap_or(0)
    };

    assert_eq!(
        count(&static_store, "main > section"),
        count(&runtime_store, "main > section")
    );
    assert_eq!(
        count(&static_store, "> a[href]"),
        count(&runtime_store, "> a[href]")
    );
    assert_eq!(count(&static_store, "span"), count(&runtime_store, "span"));
}

#[test]
fn test_macro_static_query_with_nested_attributes() {
    let not_query = query! {
        all("div:not([hidden])", Save::none())
    };
    let alternatives_query = query! {
        all("div:is([data-x=a], [data-x=b])", Save::none())
    };

    assert_eq!(not_query.states().len(), 1);
    assert_eq!(alternatives_query.states().len(), 1);
    assert_eq!(
        alternatives_query.states()[0].metadata().attribute_names(),
        &["data-x"]
    );
}

#[test]
fn test_macro_query_matches_runtime_query_structure() {
    let static_query = query! {
        all("main > section", Save::all()) => {
            all("> a[href]", Save::all()),
            first("span", Save::only_text()),
        }
    };
    let runtime_query = Query::all("main > section", Save::all())
        .unwrap()
        .then(|ctx| {
            Ok([
                ctx.all("> a[href]", Save::all())?,
                ctx.first("span", Save::only_text())?,
            ])
        })
        .unwrap()
        .build();

    assert_eq!(static_query.states().len(), runtime_query.states().len());
    for (static_state, runtime_state) in static_query.states().iter().zip(runtime_query.states()) {
        assert_eq!(static_state.guard, runtime_state.guard);
        assert_eq!(
            static_state.predicate().name,
            runtime_state.predicate().name
        );
        assert_eq!(static_state.predicate().id, runtime_state.predicate().id);
        assert_eq!(
            static_state.predicate().classes.as_slice(),
            runtime_state.predicate().classes.as_slice()
        );
        assert_eq!(
            static_state.predicate().attributes.as_slice(),
            runtime_state.predicate().attributes.as_slice()
        );
    }

    assert_eq!(static_query.queries(), runtime_query.queries());
    assert_eq!(
        static_query.exit_at_section_end(),
        runtime_query.exit_at_section_end()
    );
}

#[test]
fn test_macro_query_matches_runtime_store_contents() {
    let static_query = query! {
        all("main > section", Save::all()) => {
            all("> a[href]", Save::all()),
            first("span", Save::only_text()),
        }
    };
    let runtime_query = Query::all("main > section", Save::all())
        .unwrap()
        .then(|ctx| {
            Ok([
                ctx.all("> a[href]", Save::all())?,
                ctx.first("span", Save::only_text())?,
            ])
        })
        .unwrap()
        .build();

    let static_queries = [static_query];
    let runtime_queries = [runtime_query];
    let static_store = parse(HTML, &static_queries).unwrap();
    let runtime_store = parse(HTML, &runtime_queries).unwrap();

    type Content = Vec<(String, Option<String>, Option<String>, Option<String>)>;

    fn collect_query_contents<'html, 'query>(
        store: &Store<'html, 'query>,
        selector: &str,
    ) -> Content {
        store
            .get(selector)
            .into_iter()
            .flatten()
            .map(|element| {
                (
                    element.name.to_string(),
                    element.attribute(store, "href").map(str::to_string),
                    element.inner_html.map(str::trim).map(str::to_string),
                    element.text(store).map(str::to_string),
                )
            })
            .collect()
    }

    for selector in ["main > section", "> a[href]", "span"] {
        assert_eq!(
            collect_query_contents(&static_store, selector),
            collect_query_contents(&runtime_store, selector),
            "selector mismatch for {selector}"
        );
    }
}

#[test]
fn replacing_transition_predicate_refreshes_parser_preflight() {
    let mut query = Query::all("a", Save::name_only()).unwrap().build();
    let mut predicate = query.states[0].predicate().clone();
    predicate.name = Some("div");
    query.states[0].set_predicate(predicate);

    let queries = [query];
    let store = parse("<a></a><div></div>", &queries).unwrap();
    let mut matches = store.get("a").unwrap();

    assert_eq!(matches.next().unwrap().name, "div");
    assert!(matches.next().is_none());
}

#[test]
fn escaped_attribute_values_are_rejected() {
    // CSS decodes `\"` to `"`, so matching the raw backslash would differ from
    // a browser. Escapes are unsupported until scah decodes them.
    for selector in [r#"a[title="hello \"world\""]"#, r"a[title=hello\ world]"] {
        let error = Query::all(selector, Save::all()).err().unwrap();
        assert_eq!(
            error.message(),
            "escaped attribute values are not supported",
            "{selector}"
        );
    }
}

#[test]
fn quoted_attribute_selector_matches_url_with_query_string() {
    let html = r#"<a href="https://example.com/search?q=test">x</a>"#;
    let selector = r#"a[href="https://example.com/search?q=test"]"#;

    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn quoted_attribute_selector_matches_value_with_selector_control_chars() {
    let html = r#"<div data-x="a=b*c]d"></div>"#;
    let selector = r#"div[data-x="a=b*c]d"]"#;

    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn form_feed_descendant_combinator_matches() {
    let html = "<main><section id='s1'></section></main>";
    let selector = "main\u{000C}section";

    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn logical_pseudos_reject_unsupported_alternatives_instead_of_narrowing_results() {
    // A browser matches these selectors, so silently discarding an
    // alternative would return fewer elements than expected.
    for selector in [
        "a:is(div > a)",
        "a:not(:is(div > a))",
        "div:is(div > a, .x)",
        "a:is(:first-child)",
        "a:where(.q, :has(b))",
        "div:is(.caf\u{e9})",
    ] {
        assert!(
            Query::all(selector, Save::all()).is_err(),
            "{selector} should be rejected"
        );
    }

    let html = "<div class=x>X</div><div class=y>Y</div>";
    let queries = [Query::all("div:is(.x, .bad])", Save::all())
        .unwrap()
        .build()];
    let store = parse(html, &queries).unwrap();
    let texts: Vec<_> = store
        .get("div:is(.x, .bad])")
        .unwrap()
        .map(|element| element.text(&store))
        .collect();
    assert_eq!(texts, vec![Some("X")]);
}

const REPEATED_ID_HTML: &str =
    r#"<main><div id="hero">H</div><div id="other">O</div><div>N</div></main>"#;

const REPEATED_ID_CASES: &[(&str, &[&str])] = &[
    ("div#hero#hero", &["H"]),
    ("div#hero#other", &[]),
    ("div:is(#hero#hero)", &["H"]),
    ("div:is(#hero#other)", &[]),
    ("div:where(#hero#hero)", &["H"]),
    ("div:where(#hero#other)", &[]),
    ("div:not(#hero#hero)", &["O", "N"]),
    ("div:not(#hero#other)", &["H", "O", "N"]),
    ("div:not(:is(#hero#hero))", &["O", "N"]),
    ("div:not(:where(#hero#other))", &["H", "O", "N"]),
];

fn repeated_id_texts<'q, Q: QuerySpec<'q>>(queries: &'q [Q], selector: &str) -> Vec<String> {
    let store = parse(REPEATED_ID_HTML, queries).unwrap();
    store
        .get(selector)
        .map(|elements| {
            elements
                .map(|element| element.text(&store).unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn repeated_id_selectors_match_as_a_conjunction() {
    for &(selector, expected) in REPEATED_ID_CASES {
        let queries = [Query::all(selector, Save::all()).unwrap().build()];
        assert_eq!(
            repeated_id_texts(&queries, selector),
            expected,
            "{selector}"
        );
    }
}

#[test]
fn repeated_id_selectors_match_in_macro_queries() {
    macro_rules! check {
        ($index:literal, $selector:literal) => {{
            let (selector, expected) = REPEATED_ID_CASES[$index];
            assert_eq!(selector, $selector);
            let static_queries = [query! { all($selector, Save::all()) }];
            let runtime_query = Query::all(selector, Save::all()).unwrap().build();
            let static_states = static_queries[0].states();
            assert_eq!(static_states.len(), runtime_query.states().len());
            for (static_state, runtime_state) in static_states.iter().zip(runtime_query.states()) {
                assert_eq!(static_state.guard, runtime_state.guard, "{selector}");
                assert_eq!(
                    static_state.predicate(),
                    runtime_state.predicate(),
                    "{selector}: macro and runtime predicates differ"
                );
                assert_eq!(
                    static_state.metadata().attribute_names(),
                    runtime_state.metadata().attribute_names(),
                    "{selector}"
                );
            }
            assert_eq!(
                repeated_id_texts(&static_queries, selector),
                expected,
                "{selector}"
            );
        }};
    }

    check!(0, "div#hero#hero");
    check!(1, "div#hero#other");
    check!(2, "div:is(#hero#hero)");
    check!(3, "div:is(#hero#other)");
    check!(4, "div:where(#hero#hero)");
    check!(5, "div:where(#hero#other)");
    check!(6, "div:not(#hero#hero)");
    check!(7, "div:not(#hero#other)");
    check!(8, "div:not(:is(#hero#hero))");
    check!(9, "div:not(:where(#hero#other))");
    assert_eq!(REPEATED_ID_CASES.len(), 10);
}

#[test]
fn stray_quotes_after_selectors_are_rejected() {
    for selector in ["b[a]\"", "div:not(.a)\"", "div[class]''"] {
        assert!(
            Query::all(selector, Save::all()).is_err(),
            "{selector} should be rejected"
        );
    }
}

#[test]
fn forgiving_lists_reject_syntax_the_parser_does_not_model() {
    // Each alternative is valid CSS that scah would misread. Forgiving it as
    // invalid would make `:is()` / `:where()` miss elements a browser matches
    // and the enclosing `:not()` return extra ones, so the query is rejected.
    for inner in [
        r":\6e ot(.missing)",
        ".foo/**/.bar",
        r"[data-x=a\,b]",
        r"[data-x=a\)b]",
        r"[data-x=a\]b]",
        r".a\)b",
    ] {
        for selector in [
            format!("div{inner}"),
            format!("div:is({inner})"),
            format!("div:where({inner})"),
            format!("div:not(:is({inner}))"),
            format!("div:not(:where({inner}))"),
            format!("div:is(.x, {inner})"),
        ] {
            assert!(
                Query::all(&selector, Save::all()).is_err(),
                "{selector} should be rejected"
            );
        }
    }
}

const FORGIVING_HTML: &str = r#"<main><div class="foo bar">F</div><div data-x="a,b">C</div><div data-x="a)b">P</div><div>N</div></main>"#;

const FORGIVING_CASES: &[(&str, &[&str])] = &[
    (r#"div:is([data-x="a,b"])"#, &["C"]),
    (r#"div:where([data-x="a,b"], [data-x="a)b"])"#, &["C", "P"]),
    (r#"div:not(:is([data-x="a,b"]))"#, &["F", "P", "N"]),
    (r#"div:not(:where([data-x="a)b"], !!!))"#, &["F", "C", "N"]),
    ("div:is(.foo, !!!)", &["F"]),
    ("div:where(.bar, !!!)", &["F"]),
    ("div:not(:is(.foo, !!!))", &["C", "P", "N"]),
    ("div:not(:where(.foo, !!!))", &["C", "P", "N"]),
];

fn forgiving_texts<'q, Q: QuerySpec<'q>>(queries: &'q [Q], selector: &str) -> Vec<String> {
    let store = parse(FORGIVING_HTML, queries).unwrap();
    store
        .get(selector)
        .map(|elements| {
            elements
                .map(|element| element.text(&store).unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn forgiving_lists_keep_quoted_commas_and_forgive_invalid_branches() {
    for &(selector, expected) in FORGIVING_CASES {
        let queries = [Query::all(selector, Save::all()).unwrap().build()];
        assert_eq!(forgiving_texts(&queries, selector), expected, "{selector}");
    }
}

#[test]
fn forgiving_lists_match_in_macro_queries() {
    macro_rules! check {
        ($index:literal, $selector:literal) => {{
            let (selector, expected) = FORGIVING_CASES[$index];
            assert_eq!(selector, $selector);
            let static_queries = [query! { all($selector, Save::all()) }];
            let runtime_query = Query::all(selector, Save::all()).unwrap().build();
            let static_states = static_queries[0].states();
            assert_eq!(static_states.len(), runtime_query.states().len());
            for (static_state, runtime_state) in static_states.iter().zip(runtime_query.states()) {
                assert_eq!(
                    static_state.predicate(),
                    runtime_state.predicate(),
                    "{selector}: macro and runtime predicates differ"
                );
            }
            assert_eq!(
                forgiving_texts(&static_queries, selector),
                expected,
                "{selector}"
            );
        }};
    }

    check!(0, r#"div:is([data-x="a,b"])"#);
    check!(1, r#"div:where([data-x="a,b"], [data-x="a)b"])"#);
    check!(2, r#"div:not(:is([data-x="a,b"]))"#);
    check!(3, r#"div:not(:where([data-x="a)b"], !!!))"#);
    check!(4, "div:is(.foo, !!!)");
    check!(5, "div:where(.bar, !!!)");
    check!(6, "div:not(:is(.foo, !!!))");
    check!(7, "div:not(:where(.foo, !!!))");
    assert_eq!(FORGIVING_CASES.len(), 8);
}
