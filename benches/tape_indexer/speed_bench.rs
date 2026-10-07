//! Compares tag-indexing strategies on the same documents and queries:
//! the production policy (`parse`), the rolling and full-document packed
//! indexers, and the simdlex structural-tape indexer.
//!
//! Real pages are read from `SCAH_TAPE_CORPUS` (a directory of `.html`
//! files) when it is set.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use scah::bench_internals::{parse_with_indexing_mode, parse_with_tape_indexer};
use scah::{Query, Save, parse};
use std::hint::black_box;
use std::time::Duration;

fn documents() -> Vec<(String, String)> {
    let mut out = Vec::new();

    let mut flat = String::from("<main>");
    for _ in 0..10_000 {
        flat.push_str("<div></div>");
    }
    flat.push_str("</main>");
    out.push(("flat_divs".to_string(), flat));

    let mut rows = String::from("<table>");
    for index in 0..5_000 {
        rows.push_str(&format!(
            "<tr class=\"row\"><td data-i=\"{index}\"><a class=\"hit\" href=\"/item/{index}\">item {index}</a></td><td>{index}</td></tr>"
        ));
    }
    rows.push_str("</table>");
    out.push(("attribute_rows".to_string(), rows));

    let mut prose = String::new();
    for index in 0..1_000 {
        prose.push_str("<p>");
        prose.push_str(&"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(7));
        if index % 10 == 0 {
            prose.push_str("<a href=\"/x\">link</a>");
        }
        prose.push_str("</p>\n");
    }
    out.push(("prose".to_string(), prose));

    if let Ok(directory) = std::env::var("SCAH_TAPE_CORPUS") {
        let mut paths: Vec<_> = std::fs::read_dir(directory)
            .expect("SCAH_TAPE_CORPUS must be a directory")
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "html"))
            .collect();
        paths.sort();
        for path in paths {
            let name: String = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .chars()
                .take(20)
                .collect();
            let html = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
            out.push((name, html));
        }
    }
    out
}

fn bench_strategies(c: &mut Criterion) {
    let all_a = [Query::all("a", Save::name_only()).unwrap().build()];
    let a_href = [Query::all("a[href]", Save::name_only()).unwrap().build()];
    let first_a = [Query::first("a", Save::name_only()).unwrap().build()];
    let workloads: [(&str, &[_], &str); 3] = [
        ("all_a", &all_a, "a"),
        ("a_href", &a_href, "a[href]"),
        ("first_a", &first_a, "a"),
    ];

    for (document, html) in documents() {
        let mut group = c.benchmark_group(format!("tape_indexer/{document}"));
        group.sample_size(20);
        group.warm_up_time(Duration::from_millis(500));
        group.measurement_time(Duration::from_secs(2));
        group.throughput(Throughput::Bytes(html.len() as u64));

        for (workload, queries, selector) in workloads {
            let count =
                |store: scah::Store<'_, '_>| store.get(selector).map_or(0, |items| items.count());
            let expected = count(parse(&html, queries).unwrap());
            assert_eq!(
                count(parse_with_tape_indexer(&html, queries).unwrap()),
                expected
            );
            assert_eq!(
                count(parse_with_indexing_mode(&html, queries, true).unwrap()),
                expected
            );

            group.bench_with_input(BenchmarkId::new("policy", workload), &html, |b, html| {
                b.iter(|| count(parse(black_box(html), black_box(queries)).unwrap()))
            });
            for (strategy, full_index) in [("rolling", false), ("full_index", true)] {
                group.bench_with_input(BenchmarkId::new(strategy, workload), &html, |b, html| {
                    b.iter(|| {
                        count(
                            parse_with_indexing_mode(
                                black_box(html),
                                black_box(queries),
                                full_index,
                            )
                            .unwrap(),
                        )
                    })
                });
            }
            group.bench_with_input(BenchmarkId::new("tape", workload), &html, |b, html| {
                b.iter(|| {
                    count(parse_with_tape_indexer(black_box(html), black_box(queries)).unwrap())
                })
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_strategies);
criterion_main!(benches);
