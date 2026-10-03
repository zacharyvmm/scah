//! Differential tests: `parse_columnar` must read back exactly what `parse`
//! stores, for every element field, attribute, text value, and nested result.
//!
//! The corpus reuses the HTML and selectors of the existing test suites by
//! scanning their source for string literals, then adds generated workloads
//! and truncated (EOF) variants of every document.

use scah::{
    ColumnarParseError, ColumnarStore, Element, ElementRef, Query, QuerySpec, Save, Store, parse,
    parse_columnar,
};

const TEST_SOURCES: &[&str] = &[
    include_str!("integration_test.rs"),
    include_str!("text_extraction_test.rs"),
    include_str!("debugging_test.rs"),
    include_str!("html_soup/case_insensitive_stack_rules_test.rs"),
    include_str!("html_soup/child_combinator_test.rs"),
    include_str!("html_soup/eof_recovery_test.rs"),
    include_str!("html_soup/implied_close_test.rs"),
    include_str!("html_soup/misnesting_test.rs"),
    include_str!("html_soup/parser_edge_case_test.rs"),
    include_str!("html_soup/selector_stability_test.rs"),
    include_str!("html_soup/sibling_combinator_test.rs"),
    include_str!("html_soup/void_rawtext_test.rs"),
    include_str!("../src/html/parser.rs"),
    include_str!("../src/engine/executor.rs"),
    include_str!("../src/lib.rs"),
];

const SAVES: [Save; 8] = [
    Save {
        inner_html: true,
        raw_text: true,
        text: true,
        attributes: true,
    },
    Save {
        inner_html: false,
        raw_text: false,
        text: false,
        attributes: true,
    },
    Save {
        inner_html: false,
        raw_text: false,
        text: false,
        attributes: false,
    },
    Save {
        inner_html: true,
        raw_text: false,
        text: false,
        attributes: false,
    },
    Save {
        inner_html: false,
        raw_text: false,
        text: true,
        attributes: true,
    },
    Save {
        inner_html: false,
        raw_text: true,
        text: false,
        attributes: false,
    },
    Save {
        inner_html: true,
        raw_text: false,
        text: true,
        attributes: true,
    },
    Save {
        inner_html: false,
        raw_text: true,
        text: true,
        attributes: false,
    },
];

/// A string literal from Rust source and the code right before it.
struct Literal {
    value: String,
    context: String,
}

/// Collect string literals (plain and raw) from Rust source, skipping
/// comments and char literals.
fn string_literals(source: &str) -> Vec<Literal> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' => {
                // Char literal (`'x'`, `'\n'`) or lifetime.
                if bytes.get(i + 1) == Some(&b'\\') {
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'\'' {
                        i += 1;
                    }
                    i += 1;
                } else if let Some(c) = source[i + 1..].chars().next()
                    && source[i + 1 + c.len_utf8()..].starts_with('\'')
                {
                    i += 2 + c.len_utf8();
                } else {
                    i += 1;
                }
            }
            b'r' if (bytes.get(i + 1) == Some(&b'"') || bytes.get(i + 1) == Some(&b'#'))
                && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')) =>
            {
                let start = i;
                let mut hashes = 0;
                let mut j = i + 1;
                while bytes.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if bytes.get(j) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let terminator = format!("\"{}", "#".repeat(hashes));
                let body_start = j + 1;
                let Some(end) = source[body_start..].find(&terminator) else {
                    break;
                };
                literals.push(Literal {
                    value: source[body_start..body_start + end].to_string(),
                    context: context_before(source, start),
                });
                i = body_start + end + terminator.len();
            }
            b'"' => {
                let start = i;
                let mut value = String::new();
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                        match bytes.get(i) {
                            Some(b'n') => value.push('\n'),
                            Some(b't') => value.push('\t'),
                            Some(b'r') => value.push('\r'),
                            Some(b'0') => value.push('\0'),
                            Some(b'\n') => {
                                while i + 1 < bytes.len() && bytes[i + 1].is_ascii_whitespace() {
                                    i += 1;
                                }
                            }
                            Some(b'u') => {
                                let close = source[i..].find('}').map_or(i, |end| i + end);
                                let hex = &source[i + 2..close];
                                if let Some(c) =
                                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                                {
                                    value.push(c);
                                }
                                i = close;
                            }
                            Some(b'x') => {
                                if let Ok(byte) = u8::from_str_radix(&source[i + 1..i + 3], 16) {
                                    value.push(byte as char);
                                }
                                i += 2;
                            }
                            Some(_) => {
                                let c = source[i..].chars().next().unwrap();
                                value.push(c);
                                i += c.len_utf8() - 1;
                            }
                            None => break,
                        }
                        i += 1;
                    } else {
                        let c = source[i..].chars().next().unwrap();
                        value.push(c);
                        i += c.len_utf8();
                    }
                }
                i += 1;
                literals.push(Literal {
                    value,
                    context: context_before(source, start),
                });
            }
            _ => i += 1,
        }
    }
    literals
}

fn context_before(source: &str, end: usize) -> String {
    let mut start = end.saturating_sub(40);
    while !source.is_char_boundary(start) {
        start += 1;
    }
    source[start..end].split_whitespace().collect::<String>()
}

fn looks_like_html(value: &str) -> bool {
    value.as_bytes().windows(2).any(|pair| {
        pair[0] == b'<' && (pair[1].is_ascii_alphabetic() || matches!(pair[1], b'/' | b'!'))
    })
}

fn is_selector_context(context: &str) -> bool {
    ["all(", "first(", "&[", "[(", "\",", "query!("]
        .iter()
        .any(|suffix| context.ends_with(suffix))
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn generated_documents() -> Vec<String> {
    let mut catalog = String::from(r#"<html><body><section id="products">"#);
    for i in 1..=40 {
        let rating = (i - 1) % 5 + 1;
        catalog.push_str(&format!(
            r#"<div class="product"><h1>Product #{i}</h1><span class="rating">{rating}/5</span><p class="description">Description &amp; more</p></div>"#
        ));
    }
    catalog.push_str("</section></body></html>");

    let mut links = String::from("<html><body><div id='content'>");
    for i in 0..60 {
        links.push_str(&format!(
            r#"<div class="article"><a href="/post/{i}"><b>Post</b> &lt;{i}&gt;</a></div>"#
        ));
    }
    links.push_str("</div></body></html>");

    let deep = format!(
        "{}<p id=deep>x</p>{}",
        "<div class=d>".repeat(64),
        "</div>".repeat(64)
    );
    let unclosed = "<ul><li>a<li>b<p>c<li><a href=x>d</ul><table><tr><td>1<td>2</table>".repeat(5);
    let soup = concat!(
        "<!DOCTYPE html><html><head><title>t &amp; t</title><style>a<b{}</style>",
        "<script>if (a < b) { document.write('<a href=x>') }</script></head>",
        "<body><main><section><h1 id=a class='x y'>One</h1><p>p1<br>p2</p>",
        "<a HREF=one data-x>one</a><a href='two' href=dup>two</a></section>",
        "<section><textarea>\n <a>raw</a></textarea><pre>\n  keep  </pre>",
        "<div hidden><a href=h>hidden</a></div><img src=i alt=''><input disabled/>",
        "</section><!-- <a href=c>comment</a> --></main></body></html>"
    )
    .to_string();
    vec![catalog, links, deep, unclosed, soup]
}

fn corpus() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut documents: Vec<String> = Vec::new();
    let mut selectors: Vec<String> = Vec::new();
    for source in TEST_SOURCES {
        for literal in string_literals(source) {
            if looks_like_html(&literal.value) {
                documents.push(literal.value);
            } else if !literal.value.is_empty()
                && literal.value.len() <= 80
                && !literal.value.contains('\n')
                && is_selector_context(&literal.context)
                && Query::all(&literal.value, Save::all()).is_ok()
            {
                selectors.push(literal.value);
            }
        }
    }
    documents.extend(generated_documents());

    // Truncated variants exercise end-of-file recovery.
    let truncated: Vec<String> = documents
        .iter()
        .filter(|document| document.len() > 8)
        .flat_map(|document| {
            [document.len() / 3, document.len() * 2 / 3]
                .into_iter()
                .map(|mut cut| {
                    while !document.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    document[..cut].to_string()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    documents.extend(truncated);

    documents.sort();
    documents.dedup();
    selectors.sort();
    selectors.dedup();
    (
        documents.into_iter().map(leak).collect(),
        selectors.into_iter().map(leak).collect(),
    )
}

fn nested_query_sets() -> Vec<Vec<Query<'static>>> {
    let all = SAVES[0];
    let lean = SAVES[2];
    let text = SAVES[4];
    vec![
        vec![
            Query::all("div.product", all)
                .unwrap()
                .then(|product| {
                    Ok([
                        product.first("> h1", text)?,
                        product.first("> span.rating", all)?,
                        product.first("> p.description", lean)?,
                    ])
                })
                .unwrap()
                .build(),
        ],
        vec![
            Query::first("div.product", all)
                .unwrap()
                .then(|product| Ok([product.all("*", SAVES[3])?]))
                .unwrap()
                .build(),
        ],
        vec![
            Query::all("main > section", all)
                .unwrap()
                .then(|section| {
                    Ok([
                        section.all("> a[href]", all)?,
                        section.all("div a", SAVES[5])?,
                        section.first("p", SAVES[7])?,
                    ])
                })
                .unwrap()
                .build(),
            Query::all("a", lean).unwrap().build(),
            Query::all("a", text).unwrap().build(),
        ],
        vec![
            Query::all("div", lean)
                .unwrap()
                .then(|div| {
                    Ok([div
                        .all("div", SAVES[1])?
                        .then(|inner| Ok([inner.all("p", all)?, inner.first("a", text)?]))?])
                })
                .unwrap()
                .build(),
        ],
        vec![
            Query::all("ul", all)
                .unwrap()
                .then(|list| {
                    Ok([
                        list.all("> li", SAVES[6])?,
                        list.all("li + li", all)?,
                        list.all("li ~ li", lean)?,
                    ])
                })
                .unwrap()
                .build(),
            Query::all("h1 + p", all).unwrap().build(),
            Query::all("h1 ~ p", text).unwrap().build(),
        ],
        vec![
            Query::all("section, article", all)
                .unwrap()
                .then(|scope| {
                    Ok([
                        scope.all(":scope > *", SAVES[3])?,
                        scope.all("a:nth-child(2n+1)", all)?,
                    ])
                })
                .unwrap()
                .build(),
            Query::all("li:nth-of-type(2)", all).unwrap().build(),
        ],
        vec![
            Query::all("body", lean)
                .unwrap()
                .then(|body| {
                    Ok([
                        body.all("a", all)?,
                        body.all("a", text)?,
                        body.all("b", all)?,
                    ])
                })
                .unwrap()
                .build(),
            Query::first("p", all).unwrap().build(),
            Query::first("a", all).unwrap().build(),
        ],
    ]
}

fn sections<'q, Q: QuerySpec<'q>>(queries: &[Q]) -> Vec<&'q str> {
    let mut sources: Vec<&str> = queries
        .iter()
        .flat_map(|query| query.queries().iter().map(|section| section.source))
        .collect();
    sources.push("not-a-query-selector");
    sources.sort();
    sources.dedup();
    sources
}

fn store_id(store: &Store<'_, '_>, element: &Element<'_>) -> usize {
    let base = store.elements.as_ptr().addr();
    let offset = std::ptr::from_ref(element).addr() - base;
    assert_eq!(offset % size_of::<Element>(), 0);
    offset / size_of::<Element>()
}

fn assert_same_element<'a>(
    store: &'a Store<'a, 'a>,
    expected: &'a Element<'a>,
    actual: ElementRef<'a, 'a, 'a>,
    selectors: &[&str],
    case: &str,
) {
    let id = actual.id();
    assert_eq!(store_id(store, expected), id as usize, "{case}");
    assert_eq!(expected.name, actual.name(), "{case}: name of {id}");
    assert_eq!(expected.class, actual.class_name(), "{case}: class of {id}");
    assert_eq!(expected.id, actual.html_id(), "{case}: id of {id}");
    assert_eq!(
        expected.inner_html,
        actual.inner_html(),
        "{case}: inner html of {id}"
    );
    assert_eq!(
        expected.attributes(store),
        actual.attributes(),
        "{case}: attributes of {id}"
    );
    let mut keys: Vec<String> = ["class", "id", "href", "HREF", "missing", ""]
        .iter()
        .map(|key| key.to_string())
        .collect();
    for attribute in expected.attributes(store).unwrap_or_default() {
        keys.push(attribute.key.to_string());
        keys.push(attribute.key.to_ascii_uppercase());
    }
    for key in &keys {
        assert_eq!(
            expected.attribute(store, key),
            actual.attribute(key),
            "{case}: attribute {key:?} of {id}"
        );
    }
    assert_eq!(
        expected.raw_text(store),
        actual.raw_text(),
        "{case}: raw text of {id}"
    );
    assert_eq!(expected.text(store), actual.text(), "{case}: text of {id}");
    assert_eq!(
        expected.has_raw_text(store),
        actual.has_raw_text(),
        "{case}"
    );
    assert_eq!(expected.has_text(store), actual.has_text(), "{case}");
    for selector in selectors {
        let expected_children = expected.get(store, selector).map(|children| {
            children
                .map(|child| store_id(store, child))
                .collect::<Vec<_>>()
        });
        let actual_children = actual.get(selector).map(|children| {
            children
                .map(|child| child.id() as usize)
                .collect::<Vec<_>>()
        });
        assert_eq!(
            expected_children, actual_children,
            "{case}: nested {selector:?} under {id}"
        );
    }
}

fn assert_same_results<'a>(
    store: &'a Store<'a, 'a>,
    columnar: &'a ColumnarStore<'a, 'a>,
    selectors: &[&str],
    case: &str,
) {
    assert_eq!(
        store.elements.len(),
        columnar.len(),
        "{case}: element count"
    );
    assert_eq!(store.attributes.len(), columnar.attribute_buffer().len());
    assert_eq!(
        store.raw_text_buffer(),
        columnar.raw_text_buffer(),
        "{case}"
    );
    assert_eq!(store.text_buffer(), columnar.text_buffer(), "{case}");

    for (expected, actual) in store.elements.iter().zip(columnar.elements()) {
        assert_same_element(store, expected, actual, selectors, case);
    }
    for selector in selectors {
        let expected = store
            .get(selector)
            .map(|elements| elements.map(|e| store_id(store, e)).collect::<Vec<_>>());
        let actual = columnar
            .get(selector)
            .map(|elements| elements.map(|e| e.id() as usize).collect::<Vec<_>>());
        assert_eq!(expected, actual, "{case}: root {selector:?}");
    }
}

/// Parse with both stores and assert they read back identically. Returns
/// the number of matched elements.
fn assert_differential<'q, Q: QuerySpec<'q>>(html: &'q str, queries: &'q [Q], case: &str) -> usize {
    let selectors = sections(queries);
    match (parse(html, queries), parse_columnar(html, queries)) {
        (Ok(store), Ok(columnar)) => {
            assert_same_results(&store, &columnar, &selectors, case);
            store.elements.len()
        }
        (Err(expected), Err(ColumnarParseError::Parse(actual))) => {
            assert_eq!(expected, actual, "{case}");
            0
        }
        (expected, actual) => panic!("{case}: {expected:?} vs {actual:?}"),
    }
}

#[test]
fn corpus_is_substantial() {
    let (documents, selectors) = corpus();
    eprintln!(
        "corpus: {} documents, {} selectors",
        documents.len(),
        selectors.len()
    );
    assert!(documents.len() > 300, "{} documents", documents.len());
    assert!(selectors.len() > 100, "{} selectors", selectors.len());
}

#[test]
fn every_selector_alone_matches_on_every_document() {
    let (documents, selectors) = corpus();
    let mut matched = 0;
    for (index, selector) in selectors.iter().enumerate() {
        let save = SAVES[index % SAVES.len()];
        let queries = [Query::all(selector, save).unwrap().build()];
        for (doc, html) in documents.iter().enumerate() {
            matched += assert_differential(html, &queries, &format!("{selector:?} on doc {doc}"));
        }
    }
    assert!(matched > 10_000, "only {matched} matches compared");
}

#[test]
fn all_selectors_together_match_on_every_document() {
    let (documents, selectors) = corpus();
    for rotation in 0..SAVES.len() {
        let queries: Vec<Query> = selectors
            .iter()
            .enumerate()
            .map(|(index, selector)| {
                let save = SAVES[(index + rotation) % SAVES.len()];
                if index % 5 == 0 {
                    Query::first(selector, save).unwrap().build()
                } else {
                    Query::all(selector, save).unwrap().build()
                }
            })
            .collect();
        // Each document sees two of the eight save rotations.
        for (doc, html) in documents.iter().enumerate().skip(rotation % 4).step_by(4) {
            assert_differential(html, &queries, &format!("rotation {rotation} on doc {doc}"));
        }
    }
}

#[test]
fn nested_queries_match_on_every_document() {
    let (documents, _) = corpus();
    for (set, queries) in nested_query_sets().iter().enumerate() {
        for (doc, html) in documents.iter().enumerate() {
            assert_differential(html, queries, &format!("nested set {set} on doc {doc}"));
        }
    }
}

#[test]
fn empty_queries_fail_the_same_way() {
    let queries: [Query; 0] = [];
    assert!(matches!(
        parse_columnar("<a>", &queries),
        Err(ColumnarParseError::Parse(scah::ParseError::EmptyQueries))
    ));
}

// Opening 65k elements takes most of a minute in an unoptimized build; the
// release test run (`cargo test --release -p scah`) covers this case.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow without optimizations")]
fn excessive_depth_fails_the_same_way() {
    let html = "<div>".repeat(usize::from(u16::MAX) + 2);
    let queries = [Query::all("p", Save::none()).unwrap().build()];
    assert_differential(&html, &queries, "depth limit");
}

#[test]
fn static_queries_match() {
    let html = "<main><section><a href='x'>Link</a><a href='y'>Two</a></section></main>";
    let query =
        scah::query! { all("main > section", Save::all()) => { all("> a[href]", Save::all()) } };
    let queries = [query];
    assert_eq!(assert_differential(html, &queries, "static query"), 3);
}
