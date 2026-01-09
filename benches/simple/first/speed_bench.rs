use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use lexbor_css::HtmlDocument;
use onego::{Query, Save, fake_parse, parse};
use scraper::{Html, Selector};
use std::hint::black_box;
use tl::ParserOptions;

const QUERY:&str = black_box("a");

fn generate_html(count: usize) -> String {
    let mut html = String::with_capacity(count * 100);
    html.push_str("<html><body><div id='content'>");
    for i in 0..count {
        // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
        html.push_str(&format!(
            r#"<div class="article"><a href="/post/{}"><b>Post</b> &lt;{}&gt;</a></div>"#,
            i, i
        ));
    }
    html.push_str("</div></body></html>");
    html
}

fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_first_selection_comparison");

    for size in [100, 1_000, 10_000].iter() {
        let content = generate_html(*size);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("onego", size),
            &content,
            |b, html| {
                b.iter(|| {
                    let queries = &[Query::first(black_box(QUERY), Save::all()).build()];

                    let res = parse(&html, queries);

                    let element = res[QUERY].value().unwrap();
                    black_box(&element.attributes);
                    black_box(&element.inner_html);
                    black_box(&element.text_content);
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("tl", size),
            &content,
            |b, html| {
                b.iter(|| {
                    let dom = tl::parse(html, ParserOptions::default()).unwrap();
                    let parser = dom.parser();
                    let node_handle = dom.query_selector(QUERY).unwrap().next().unwrap();

                    if let Some(node) = node_handle.get(parser) {
                        let attributes = node.as_tag().unwrap().attributes();
                        black_box(attributes.get("href"));
                        black_box(node.inner_html(parser));
                        black_box(node.inner_text(parser));
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scraper", size),
            &content,
            |b, html| {
                b.iter(|| {
                    let document = Html::parse_document(html);
                    let selector = Selector::parse(QUERY).unwrap();

                    let element = document.select(&selector).next().unwrap();
                    black_box(element.attr("href"));
                    black_box(element.inner_html());
                    black_box(element.text().collect::<Vec<&str>>());
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("lexbor", size),
            &content,
            |b, html| {
                b.iter(|| {
                    let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
                    let nodes = doc.select(QUERY);

                    let node = nodes.iter().next().unwrap();
                    // TODO: I need to add attributes and innerhtml for lexbor
                    black_box(node.text_content());
                    black_box(node.inner_html());
                    black_box(node.attributes());
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_comparison);
criterion_main!(benches);
