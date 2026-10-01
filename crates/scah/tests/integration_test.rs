use std::ops::Deref;

#[cfg(debug_assertions)]
use scah::debug;
use scah::{
    AnPlusB, Attribute, ClassSelections, ElementPredicate, LocalLogicalPredicate,
    LocalSelectorList, LogicalPredicates, Query, QuerySpec, Save, Store, StructuralPredicate,
    StructuralPredicates, parse, query,
};
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
fn structural_selectors_use_streaming_child_and_type_ordinals() {
    let html =
        "<ul><li>a</li><li class='hit'>b</li><li>c</li><li class='hit'>d</li></ul><p>x</p><p>y</p>";
    let queries = [
        Query::all("li:nth-child(even)", Save::all())
            .unwrap()
            .build(),
        Query::all("li:nth-of-type(2)", Save::all())
            .unwrap()
            .build(),
        Query::all("li:nth-child(-n+3)", Save::all())
            .unwrap()
            .build(),
        Query::all("li:nth-child(2 of .hit)", Save::all())
            .unwrap()
            .build(),
    ];
    let store = parse(html, &queries).unwrap();
    assert_eq!(store.get("li:nth-child(even)").unwrap().count(), 2);
    assert_eq!(store.get("li:nth-of-type(2)").unwrap().count(), 1);
    assert_eq!(store.get("li:nth-child(-n+3)").unwrap().count(), 3);
    assert_eq!(store.get("li:nth-child(2 of .hit)").unwrap().count(), 1);
}

#[test]
fn filtered_structural_selector_has_macro_parity() {
    let html = "<ul><li class='hit'>a</li><li>b</li><li class='hit'>c</li></ul>";
    let runtime = Query::all("li:nth-child(2 of .hit)", Save::all())
        .unwrap()
        .build();
    let compiled = query! { all("li:nth-child(2 of .hit)", Save::all()) };
    let runtime_queries = [runtime];
    let compiled_queries = [compiled];
    let runtime_store = parse(html, &runtime_queries).unwrap();
    let compiled_store = parse(html, &compiled_queries).unwrap();
    assert_eq!(
        runtime_store
            .get("li:nth-child(2 of .hit)")
            .unwrap()
            .count(),
        compiled_store
            .get("li:nth-child(2 of .hit)")
            .unwrap()
            .count()
    );
}

#[test]
fn filtered_ordinals_support_multiple_filters_and_attribute_filters() {
    let html = "<ul><li class='a'>1</li><li data-card='yes'>2</li><li class='a'>3</li><li data-card='yes'>4</li></ul>";
    let queries = [
        Query::all("li:nth-child(2 of .a)", Save::all())
            .unwrap()
            .build(),
        Query::all("li:nth-child(2 of [data-card])", Save::all())
            .unwrap()
            .build(),
    ];
    let store = parse(html, &queries).unwrap();
    assert_eq!(store.get("li:nth-child(2 of .a)").unwrap().count(), 1);
    assert_eq!(
        store.get("li:nth-child(2 of [data-card])").unwrap().count(),
        1
    );
}

#[test]
fn root_selector_matches_only_the_first_document_element() {
    let html = "<!-- comment --><main>one</main><aside>two</aside>";
    let query = Query::all(":root", Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    assert_eq!(store.get(":root").unwrap().count(), 1);
}

#[test]
fn future_dependent_structural_pseudos_are_rejected() {
    assert!(Query::all("li:nth-last-child(2)", Save::none()).is_err());
    assert!(Query::all("div:has(a)", Save::none()).is_err());
}

#[test]
fn scope_selector_anchors_nested_child_queries() {
    let query = Query::all("section", Save::none())
        .unwrap()
        .then(|_| Ok([Query::all(":scope > a", Save::all()).unwrap()]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(
        "<section><a>one</a><div><a>two</a></div></section>",
        &queries,
    )
    .unwrap();
    let section = store.get("section").unwrap().next().unwrap();
    assert_eq!(section.get(&store, ":scope > a").unwrap().count(), 1);
}

#[test]
fn nested_scope_requires_a_following_relative_selector() {
    for selector in [":scope", ":scope.foo", ":scope, a"] {
        let chained = Query::all("section", Save::none())
            .unwrap()
            .all(selector, Save::none());
        assert!(chained.is_err(), "chained {selector:?} must be rejected");

        let factory = Query::all("section", Save::none())
            .unwrap()
            .then(|scope| Ok([scope.all(selector, Save::none())?]));
        assert!(factory.is_err(), "factory {selector:?} must be rejected");
    }

    for selector in [":scope > a", ":scope a"] {
        let chained = Query::all("section", Save::none())
            .unwrap()
            .all(selector, Save::none());
        assert!(chained.is_ok(), "chained {selector:?} must be accepted");

        let factory = Query::all("section", Save::none())
            .unwrap()
            .then(|scope| Ok([scope.all(selector, Save::none())?]));
        assert!(factory.is_ok(), "factory {selector:?} must be accepted");
    }
}

#[test]
fn scope_anchor_is_normalized_once_across_builder_forms() {
    let selector = ":scope > :scope > a";
    let chained = Query::all("main", Save::all())
        .unwrap()
        .all(selector, Save::all())
        .unwrap()
        .build();
    let factory = Query::all("main", Save::all())
        .unwrap()
        .then(|context| Ok([context.all(selector, Save::all())?]))
        .unwrap()
        .build();

    for query in [chained, factory] {
        let queries = [query];
        let store = parse("<main><a>unexpected</a></main>", &queries).unwrap();
        let main = store.get("main").unwrap().next().unwrap();
        assert_eq!(main.get(&store, selector).into_iter().flatten().count(), 0);
    }

    let static_query = query! {
        all("main", Save::all()) => {
            all(":scope > :scope > a", Save::all())
        }
    };
    let store = parse(
        "<main><a>unexpected</a></main>",
        std::slice::from_ref(&static_query),
    )
    .unwrap();
    let main = store.get("main").unwrap().next().unwrap();
    assert_eq!(main.get(&store, selector).into_iter().flatten().count(), 0);
}

#[test]
fn explicit_child_scope_is_not_the_parent_anchor() {
    let selector = "> :scope > a";
    let chained = Query::all("main", Save::all())
        .unwrap()
        .all(selector, Save::all())
        .unwrap()
        .build();
    let factory = Query::all("main", Save::all())
        .unwrap()
        .then(|context| Ok([context.all(selector, Save::all())?]))
        .unwrap()
        .build();
    let static_query = query! {
        all("main", Save::all()) => {
            all("> :scope > a", Save::all())
        }
    };

    for query in [chained, factory] {
        let queries = [query];
        let store = parse("<main><a>unexpected</a></main>", &queries).unwrap();
        let main = store.get("main").unwrap().next().unwrap();
        assert_eq!(main.get(&store, selector).into_iter().flatten().count(), 0);
    }
    let store = parse(
        "<main><a>unexpected</a></main>",
        std::slice::from_ref(&static_query),
    )
    .unwrap();
    let main = store.get("main").unwrap().next().unwrap();
    assert_eq!(main.get(&store, selector).into_iter().flatten().count(), 0);
}

#[test]
fn non_css_whitespace_is_not_trimmed_from_selectors() {
    for selector in ["\u{00a0}div", "div\u{00a0}", "a, \u{2003}div"] {
        assert!(
            Query::all(selector, Save::none()).is_err(),
            "{selector:?} must be rejected"
        );
    }

    // U+00A0 is an identifier code point, so the alternative is neither
    // trimmed into `div` nor discarded. scah does not support non-ASCII
    // identifiers, so the whole selector is rejected.
    assert!(Query::all(":is(\u{00a0}div, .card)", Save::name_only()).is_err());
}

#[test]
fn selector_nesting_budget_boundary_is_accepted_by_every_builder() {
    let depth = scah::MAX_SELECTOR_NESTING_DEPTH;
    let selector = format!("{}div{}", ":is(".repeat(depth), ")".repeat(depth));
    let html = "<main><div>hit</div></main>";

    let runtime = Query::all(&selector, Save::none()).unwrap().build();
    let (_tape, lazy) = unsafe {
        scah::lazy::LazyQuery::all(selector.as_str(), Save::none())
            .try_to_query()
            .unwrap()
    };
    let static_query = query! { all(":is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(:is(div))))))))))))))))))))))))))))))))", Save::none()) };
    assert_eq!(static_query.queries[0].source, selector);

    for query in [runtime, lazy] {
        let queries = [query];
        let store = parse(html, &queries).unwrap();
        assert_eq!(store.get(&selector).unwrap().count(), 1);
    }
    let store = parse(html, std::slice::from_ref(&static_query)).unwrap();
    assert_eq!(store.get(&selector).unwrap().count(), 1);

    let too_deep = format!(":is({selector})");
    let error = Query::all(&too_deep, Save::none()).err().unwrap();
    assert_eq!(
        error.message(),
        "selector nesting exceeds the maximum depth"
    );
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
fn test_top_level_scope_anchors_to_document_root() {
    let html = r#"<main><a id="child"></a></main><a id="top"></a>"#;
    let query = Query::all(":scope > a", Save::none()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    let ids = store
        .get(":scope > a")
        .unwrap()
        .filter_map(|element| element.id)
        .collect::<Vec<_>>();

    assert_eq!(ids, ["child"]);
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
fn replacing_transition_predicate_collects_nested_structural_requirements() {
    fn nested_predicate(structural: StructuralPredicate<'static>) -> ElementPredicate<'static> {
        ElementPredicate {
            name: Some("li"),
            id: None,
            classes: Default::default(),
            attributes: Default::default(),
            logical: Default::default(),
            structural: StructuralPredicates::from(vec![structural]),
        }
    }

    fn count_nested(logical: LocalLogicalPredicate<'static>) -> usize {
        let mut query = Query::all("li", Save::all()).unwrap().build();
        query.states[0].set_predicate(ElementPredicate {
            name: Some("li"),
            id: None,
            classes: Default::default(),
            attributes: Default::default(),
            logical: LogicalPredicates::from(vec![logical]),
            structural: Default::default(),
        });
        let queries = [query];
        let store = parse(
            "<ul><li class='hit'></li><li></li><li class='hit'></li></ul>",
            &queries,
        )
        .unwrap();
        store.get("li").unwrap().count()
    }

    let first_child = LocalLogicalPredicate::Any(LocalSelectorList::Owned(
        vec![nested_predicate(StructuralPredicate::FirstChild)].into_boxed_slice(),
    ));
    assert_eq!(count_nested(first_child), 1);

    let not_first_child = LocalLogicalPredicate::Not(LocalSelectorList::Owned(
        vec![nested_predicate(StructuralPredicate::FirstChild)].into_boxed_slice(),
    ));
    assert_eq!(count_nested(not_first_child), 2);

    let second_of_type = LocalLogicalPredicate::Any(LocalSelectorList::Owned(
        vec![nested_predicate(StructuralPredicate::NthOfType(AnPlusB {
            a: 0,
            b: 2,
        }))]
        .into_boxed_slice(),
    ));
    assert_eq!(count_nested(second_of_type), 1);

    let filter = LocalSelectorList::Owned(
        vec![ElementPredicate {
            name: None,
            id: None,
            classes: ClassSelections::from(vec!["hit"]),
            attributes: Default::default(),
            logical: Default::default(),
            structural: Default::default(),
        }]
        .into_boxed_slice(),
    );
    let filtered_child = LocalLogicalPredicate::Any(LocalSelectorList::Owned(
        vec![nested_predicate(StructuralPredicate::NthChildOf(
            AnPlusB { a: 0, b: 2 },
            filter,
        ))]
        .into_boxed_slice(),
    ));
    assert_eq!(count_nested(filtered_child), 1);
}

#[test]
fn filtered_ordinals_with_structural_filters_fail_closed() {
    fn li(structural: Vec<StructuralPredicate<'static>>) -> ElementPredicate<'static> {
        ElementPredicate {
            name: Some("li"),
            id: None,
            classes: Default::default(),
            attributes: Default::default(),
            logical: Default::default(),
            structural: StructuralPredicates::from(structural),
        }
    }

    fn count(position: i32) -> usize {
        // Equivalent to `li:nth-child(<position> of li:first-child)`, which
        // the selector parser rejects.
        let filter = LocalSelectorList::Owned(
            vec![li(vec![StructuralPredicate::FirstChild])].into_boxed_slice(),
        );
        let mut query = Query::all("li", Save::all()).unwrap().build();
        query.states[0].set_predicate(li(vec![StructuralPredicate::NthChildOf(
            AnPlusB { a: 0, b: position },
            filter,
        )]));
        let queries = [query];
        let store = parse("<ul><li></li><li></li><li></li></ul>", &queries).unwrap();
        store.get("li").map_or(0, |elements| elements.count())
    }

    // Counting every sibling would wrongly match the second <li>.
    assert_eq!(count(2), 0);
    assert_eq!(count(1), 0);

    // The checked setter rejects the same predicate up front.
    let filter = LocalSelectorList::Owned(
        vec![li(vec![StructuralPredicate::FirstChild])].into_boxed_slice(),
    );
    let mut query = Query::all("li", Save::all()).unwrap().build();
    let original = query.states[0].predicate().clone();
    let error = query.states[0]
        .try_set_predicate(li(vec![StructuralPredicate::NthChildOf(
            AnPlusB { a: 0, b: 2 },
            filter,
        )]))
        .unwrap_err();
    assert_eq!(
        error.message(),
        "structural pseudo-classes are not supported inside local selector lists"
    );
    assert_eq!(query.states[0].predicate(), &original);
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
fn universal_and_attribute_case_flags_match_without_lowercasing() {
    let html =
        r#"<main><div data-kind="FooBar" class="card"></div><div data-kind="other"></div></main>"#;
    let selectors = [
        "*",
        "*.card",
        r#"[data-kind="FOO" i]"#,
        r#"[data-kind^="FOO" i]"#,
        r#"[data-kind="FOO" s]"#,
    ];
    let queries: Vec<_> = selectors
        .iter()
        .map(|selector| Query::all(selector, Save::all()).unwrap().build())
        .collect();
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get("*").unwrap().count(), 3);
    assert_eq!(store.get("*.card").unwrap().count(), 1);
    assert_eq!(
        store
            .get(r#"[data-kind="FOO" i]"#)
            .map_or(0, |items| items.count()),
        0
    );
    assert_eq!(store.get(r#"[data-kind^="FOO" i]"#).unwrap().count(), 1);
    assert_eq!(
        store
            .get(r#"[data-kind="FOO" s]"#)
            .map_or(0, |items| items.count()),
        0
    );
}

#[test]
fn attribute_case_flags_have_macro_parity() {
    let query = query! { all(r#"[data-kind="FOO" i]"#, Save::all()) };
    let queries = [query];
    let store = parse(r#"<div data-kind="foo"></div>"#, &queries).unwrap();
    assert_eq!(store.get(r#"[data-kind="FOO" i]"#).unwrap().count(), 1);
}

#[test]
fn local_logical_pseudos_match_runtime_and_nested_lists() {
    let html = r#"<main><div class="ok"></div><div class="ad"></div><span hidden></span></main>"#;
    let selectors = [
        "div:not(.ad)",
        "div:is(.ok, .missing)",
        "div:where(.missing, .ad)",
        "div:not(:is(.ad, [hidden]))",
        "div:not([hidden])",
    ];
    let queries: Vec<_> = selectors
        .iter()
        .map(|selector| Query::all(selector, Save::all()).unwrap().build())
        .collect();
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get(selectors[0]).unwrap().count(), 1);
    assert_eq!(store.get(selectors[1]).unwrap().count(), 1);
    assert_eq!(store.get(selectors[2]).unwrap().count(), 1);
    assert_eq!(store.get(selectors[3]).unwrap().count(), 1);
    assert_eq!(store.get(selectors[4]).unwrap().count(), 2);
}

#[test]
fn local_logical_pseudos_have_macro_parity() {
    let query = query! { all("div:not(.ad)", Save::all()) };
    let queries = [query];
    let store = parse(r#"<div class="ok"></div><div class="ad"></div>"#, &queries).unwrap();
    assert_eq!(store.get("div:not(.ad)").unwrap().count(), 1);
}

#[test]
fn selector_lists_match_in_document_order_and_deduplicate() {
    let html =
        r#"<main><h2>two</h2><h1>one</h1><div class="hit"></div><h1 class="hit">last</h1></main>"#;
    let selector = "h1, h2";
    let query = Query::all(selector, Save::only_text()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    let names: Vec<_> = store
        .get(selector)
        .unwrap()
        .map(|element| element.name)
        .collect();
    assert_eq!(names, vec!["h2", "h1", "h1"]);

    let complex = "main > h1, main > h2";
    let query = Query::first(complex, Save::only_text()).unwrap().build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    assert_eq!(store.get(complex).unwrap().next().unwrap().name, "h2");

    let overlap = "div, .hit";
    let query = Query::all(overlap, Save::none()).unwrap().build();
    let queries = [query];
    let store = parse(r#"<div class="hit"></div><p class="hit"></p>"#, &queries).unwrap();
    assert_eq!(store.get(overlap).unwrap().count(), 2);
}

#[test]
fn selector_list_has_macro_parity() {
    let query = query! { all("h1, h2", Save::only_text_content()) };
    let queries = [query];
    let store = parse("<h2></h2><h1></h1>", &queries).unwrap();
    assert_eq!(store.get("h1, h2").unwrap().count(), 2);
}

#[test]
fn child_selector_lists_share_one_output_parent() {
    let query = Query::all("main", Save::all())
        .unwrap()
        .then(|main| Ok([main.all("h1, h2", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse("<main><h2></h2><h1></h1></main>", &queries).unwrap();
    let main = store.get("main").unwrap().next().unwrap();
    assert_eq!(main.get(&store, "h1, h2").unwrap().count(), 2);
}

#[test]
fn universal_child_selector_keeps_its_left_hand_transition() {
    let selector = "* > a";
    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse("<section><div><a></a></div></section>", &queries).unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn filtered_ordinals_exclude_elements_outside_the_filter() {
    let html = concat!(
        "<ul>",
        "<li>miss one</li><li class='hit'>hit one</li>",
        "<li>miss two</li><li class='hit'>hit two</li>",
        "</ul>"
    );
    let selectors = ["li:nth-child(n of .hit)", "li:nth-child(even of .hit)"];
    let queries: Vec<_> = selectors
        .iter()
        .map(|selector| Query::all(selector, Save::all()).unwrap().build())
        .collect();
    let store = parse(html, &queries).unwrap();

    assert_eq!(store.get(selectors[0]).unwrap().count(), 2);
    assert_eq!(store.get(selectors[1]).unwrap().count(), 1);
}

#[test]
fn filtered_ordinals_support_more_than_eight_distinct_filters() {
    let selectors: Vec<_> = (0..9)
        .map(|index| format!("li:nth-child(1 of .f{index})"))
        .collect();
    let queries: Vec<_> = selectors
        .iter()
        .map(|selector| Query::all(selector, Save::all()).unwrap().build())
        .collect();
    let html = (0..9)
        .map(|index| format!("<li class='f{index}'></li>"))
        .collect::<String>();
    let store = parse(&html, &queries).unwrap();

    for selector in &selectors {
        assert_eq!(store.get(selector).unwrap().count(), 1, "{selector}");
    }
}

#[test]
fn filtered_ordinals_support_more_than_eight_overlapping_filters() {
    let selectors: Vec<_> = (0..9)
        .map(|index| format!("li:nth-child(1 of .f{index})"))
        .collect();
    let queries: Vec<_> = selectors
        .iter()
        .map(|selector| Query::all(selector, Save::all()).unwrap().build())
        .collect();
    let classes = (0..9)
        .map(|index| format!("f{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let html = format!("<ul><li class='{classes}'></li></ul>");
    let store = parse(&html, &queries).unwrap();

    for selector in &selectors {
        assert_eq!(store.get(selector).unwrap().count(), 1, "{selector}");
    }
}

#[test]
fn filtered_ordinal_uses_one_selector_list() {
    let selector = "li:nth-child(2 of .a, [data-card])";
    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(
        "<ul><li class='a'></li><li></li><li data-card></li></ul>",
        &queries,
    )
    .unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn filtered_ordinals_count_matching_siblings_of_other_types() {
    let selector = "li:nth-child(2 of .hit)";
    let query = Query::all(selector, Save::all()).unwrap().build();
    let queries = [query];
    let store = parse(
        "<section><div class='hit'></div><li class='hit'></li></section>",
        &queries,
    )
    .unwrap();

    assert_eq!(store.get(selector).unwrap().count(), 1);
}

#[test]
fn overlapping_parent_alternatives_reuse_the_saved_output_scope() {
    let query = Query::all("div, .hit", Save::all())
        .unwrap()
        .then(|parent| Ok([parent.all("span", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse("<div class='hit'><span></span></div>", &queries).unwrap();
    let parent = store.get("div, .hit").unwrap().next().unwrap();

    assert_eq!(parent.get(&store, "span").unwrap().count(), 1);
    assert_eq!(store.get("span").map_or(0, Iterator::count), 0);
}

#[test]
fn uppercase_attribute_flags_have_runtime_and_macro_parity() {
    let runtime_insensitive = Query::all(r#"[data-x="FOO" I]"#, Save::all())
        .unwrap()
        .build();
    let runtime_sensitive = Query::all(r#"[data-x="FOO" S]"#, Save::all())
        .unwrap()
        .build();
    let compiled_insensitive = query! { all(r#"[data-x="FOO" I]"#, Save::all()) };
    let compiled_sensitive = query! { all(r#"[data-x="FOO" S]"#, Save::all()) };
    let runtime_queries = [runtime_insensitive, runtime_sensitive];
    let compiled_queries = [compiled_insensitive, compiled_sensitive];
    let runtime_store = parse("<div data-x='foo'></div>", &runtime_queries).unwrap();
    let compiled_store = parse("<div data-x='foo'></div>", &compiled_queries).unwrap();

    for store in [&runtime_store, &compiled_store] {
        assert_eq!(store.get(r#"[data-x="FOO" I]"#).unwrap().count(), 1);
        assert_eq!(
            store.get(r#"[data-x="FOO" S]"#).map_or(0, Iterator::count),
            0
        );
    }
}

#[test]
fn unquoted_i_and_s_values_have_runtime_and_macro_parity() {
    let html = "<div data-i='i' data-s='s' data-mi='I' data-ms='s'></div>";
    let runtime_queries = [
        Query::all("[data-i=i]", Save::all()).unwrap().build(),
        Query::all("[data-s=s]", Save::all()).unwrap().build(),
        Query::all("[data-mi=i i]", Save::all()).unwrap().build(),
        Query::all("[data-ms=s s]", Save::all()).unwrap().build(),
    ];
    let compiled_queries = [
        query! { all("[data-i=i]", Save::all()) },
        query! { all("[data-s=s]", Save::all()) },
        query! { all("[data-mi=i i]", Save::all()) },
        query! { all("[data-ms=s s]", Save::all()) },
    ];
    let runtime_store = parse(html, &runtime_queries).unwrap();
    let compiled_store = parse(html, &compiled_queries).unwrap();

    for selector in ["[data-i=i]", "[data-s=s]", "[data-mi=i i]", "[data-ms=s s]"] {
        assert_eq!(
            runtime_store.get(selector).unwrap().count(),
            1,
            "{selector}"
        );
        assert_eq!(
            compiled_store.get(selector).unwrap().count(),
            1,
            "{selector}"
        );
    }
}

#[test]
fn mixed_case_pseudo_names_have_runtime_and_macro_parity() {
    let html =
        "<main><div class='card'></div><div class='ad'></div><ul><li></li><li></li></ul></main>";
    let runtime_queries = [
        Query::all("li:FIRST-CHILD", Save::all()).unwrap().build(),
        Query::all("li:NTH-CHILD(2)", Save::all()).unwrap().build(),
        Query::all("div:NOT(.ad)", Save::all()).unwrap().build(),
        Query::all(":ROOT", Save::all()).unwrap().build(),
    ];
    let compiled_queries = [
        query! { all("li:FIRST-CHILD", Save::all()) },
        query! { all("li:NTH-CHILD(2)", Save::all()) },
        query! { all("div:NOT(.ad)", Save::all()) },
        query! { all(":ROOT", Save::all()) },
    ];
    let runtime_store = parse(html, &runtime_queries).unwrap();
    let compiled_store = parse(html, &compiled_queries).unwrap();

    for selector in ["li:FIRST-CHILD", "li:NTH-CHILD(2)", "div:NOT(.ad)", ":ROOT"] {
        assert_eq!(
            runtime_store.get(selector).unwrap().count(),
            compiled_store.get(selector).unwrap().count(),
            "{selector}"
        );
        assert_eq!(
            runtime_store.get(selector).unwrap().count(),
            1,
            "{selector}"
        );
    }
}

#[test]
fn an_plus_b_case_and_whitespace_have_runtime_and_macro_parity() {
    let html = "<ul><li></li><li></li><li></li><li></li></ul>";
    let runtime_queries = [
        Query::all("li:nth-child(ODD)", Save::all())
            .unwrap()
            .build(),
        Query::all("li:nth-child(2N + 1)", Save::all())
            .unwrap()
            .build(),
    ];
    let compiled_queries = [
        query! { all("li:nth-child(ODD)", Save::all()) },
        query! { all("li:nth-child(2N + 1)", Save::all()) },
    ];
    let runtime_store = parse(html, &runtime_queries).unwrap();
    let compiled_store = parse(html, &compiled_queries).unwrap();

    for selector in ["li:nth-child(ODD)", "li:nth-child(2N + 1)"] {
        assert_eq!(
            runtime_store.get(selector).unwrap().count(),
            2,
            "{selector}"
        );
        assert_eq!(
            compiled_store.get(selector).unwrap().count(),
            2,
            "{selector}"
        );
    }

    for selector in [
        "li:nth-child(3 n)",
        "li:nth-child(+ 2n)",
        "li:nth-child(+ 2)",
    ] {
        assert!(Query::all(selector, Save::none()).is_err(), "{selector}");
    }
}

#[test]
fn unsupported_structural_compositions_fail_at_query_build_time() {
    for selector in [
        "li:not(:first-child)",
        "li:nth-child(2 of :first-child)",
        ":scope.foo > a",
        // Forgiving lists must not silently discard a valid alternative.
        "li:is(:first-child)",
        "li:where(.a, :nth-child(2))",
    ] {
        assert!(Query::all(selector, Save::none()).is_err(), "{selector}");
    }
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

#[test]
fn stray_quotes_after_selectors_are_rejected() {
    for selector in ["b[a]\"", "div:not(.a)\"", "div[class]''"] {
        assert!(
            Query::all(selector, Save::all()).is_err(),
            "{selector} should be rejected"
        );
    }
}

/// Root ids paired with the ids of their `a` children, in document order.
fn root_alternative_owners(
    store: &Store<'_, '_>,
    root: &str,
    child: &str,
) -> Vec<(String, Vec<String>)> {
    store
        .get(root)
        .map(|roots| {
            roots
                .map(|parent| {
                    let children = parent
                        .get(store, child)
                        .map(|it| it.map(|a| a.id.unwrap().to_string()).collect())
                        .unwrap_or_default();
                    (parent.id.unwrap().to_string(), children)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn owners(expected: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    expected
        .iter()
        .map(|(id, children)| {
            (
                id.to_string(),
                children.iter().map(|c| c.to_string()).collect(),
            )
        })
        .collect()
}

fn parse_root_alternatives(selector: &str, html: &str) -> Vec<(String, Vec<String>)> {
    let query = Query::all(selector, Save::all())
        .unwrap()
        .then(|parent| Ok([parent.all("a", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    assert_eq!(store.get("a").map_or(0, Iterator::count), 0);
    root_alternative_owners(&store, selector, "a")
}

#[test]
fn root_alternatives_keep_root_parent_after_nested_close() {
    let html = "<div id='outer'><div id='inner'></div><span id='s'></span></div>";
    let query = Query::all("span, div", Save::none())
        .unwrap()
        .all("a", Save::none())
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    let ids: Vec<_> = store.get("span, div").unwrap().map(|e| e.id).collect();
    assert_eq!(ids.len(), 3);

    let saved_all = parse_root_alternatives("span, div", html);
    assert_eq!(
        saved_all,
        owners(&[("outer", &[]), ("inner", &[]), ("s", &[])])
    );
}

#[test]
fn root_alternatives_keep_child_ownership_in_either_order() {
    let html = concat!(
        "<div id='outer'><a id='a1'></a>",
        "<div id='inner'><a id='a2'></a></div>",
        "<span id='s1'><a id='a3'></a></span>",
        "<a id='a4'></a></div>",
        "<span id='s2'><a id='a5'></a></span>",
    );
    let expected = owners(&[
        ("outer", &["a1", "a2", "a3", "a4"]),
        ("inner", &["a2"]),
        ("s1", &["a3"]),
        ("s2", &["a5"]),
    ]);

    assert_eq!(parse_root_alternatives("span, div", html), expected);
    assert_eq!(parse_root_alternatives("div, span", html), expected);
}

#[test]
fn root_alternatives_keep_parent_across_deep_closes() {
    let html = concat!(
        "<section id='sec'>",
        "<div id='d1'><div id='d2'><span id='s1'><a id='a1'></a></span></div>",
        "<p id='p1'><a id='a2'></a></p></div>",
        "<span id='s2'><div id='d3'><a id='a3'></a></div></span>",
        "</section>",
        "<p id='p2'><a id='a4'></a></p>",
    );
    let expected = owners(&[
        ("d1", &["a1", "a2"]),
        ("d2", &["a1"]),
        ("s1", &["a1"]),
        ("p1", &["a2"]),
        ("s2", &["a3"]),
        ("d3", &["a3"]),
        ("p2", &["a4"]),
    ]);

    assert_eq!(parse_root_alternatives("span, div, p", html), expected);
    assert_eq!(parse_root_alternatives("p, div, span", html), expected);
}

#[test]
fn root_alternatives_keep_parent_for_first_child_queries() {
    let html = concat!(
        "<div id='outer'><div id='inner'><a id='a1'></a></div>",
        "<span id='s'><a id='a2'></a><a id='a3'></a></span></div>",
    );
    let query = Query::all("span, div", Save::all())
        .unwrap()
        .then(|parent| Ok([parent.first("a", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(
        root_alternative_owners(&store, "span, div", "a"),
        owners(&[("outer", &["a1"]), ("inner", &["a1"]), ("s", &["a2"])])
    );
}

#[test]
fn first_root_alternative_owns_all_nested_children() {
    let html = concat!(
        "<div id='outer'><div id='inner'><a id='a1'></a></div>",
        "<span id='s'><a id='a2'></a></span></div>",
        "<span id='late'><a id='a3'></a></span>",
    );
    let query = Query::first("span, div", Save::all())
        .unwrap()
        .then(|parent| Ok([parent.all("a", Save::all())?]))
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();

    assert_eq!(
        root_alternative_owners(&store, "span, div", "a"),
        owners(&[("outer", &["a1", "a2"])])
    );
}

#[test]
fn nested_child_alternatives_keep_parent_after_nested_close() {
    let html = concat!(
        "<main id='m1'><div id='outer'><div id='inner'><a id='a1'></a></div>",
        "<span id='s'><a id='a2'></a></span></div></main>",
        "<main id='m2'><span id='s2'></span></main>",
    );
    let query = Query::all("main", Save::all())
        .unwrap()
        .then(|main| {
            Ok([main
                .all("span, div", Save::all())?
                .then(|parent| Ok([parent.all("a", Save::all())?]))?])
        })
        .unwrap()
        .build();
    let queries = [query];
    let store = parse(html, &queries).unwrap();
    let mains: Vec<_> = store.get("main").unwrap().collect();
    assert_eq!(mains.len(), 2);

    let nested = |main: &scah::Element<'_>| -> Vec<(String, Vec<String>)> {
        main.get(&store, "span, div")
            .map(|parents| {
                parents
                    .map(|parent| {
                        let children = parent
                            .get(&store, "a")
                            .map(|it| it.map(|a| a.id.unwrap().to_string()).collect())
                            .unwrap_or_default();
                        (parent.id.unwrap().to_string(), children)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    assert_eq!(
        nested(mains[0]),
        owners(&[("outer", &["a1", "a2"]), ("inner", &["a1"]), ("s", &["a2"])])
    );
    assert_eq!(nested(mains[1]), owners(&[("s2", &[])]));
}
