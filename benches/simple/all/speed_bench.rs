use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use lexbor_css::HtmlDocument;
use lol_html::{HtmlRewriter, Settings, element};
use onego::{Query, Save, fake_parse, parse};
use scraper::{Html, Selector};
use std::hint::black_box;
use tl::ParserOptions;

const QUERY: &str = black_box("a");

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
    let mut group = c.benchmark_group("simple_all_selection_comparison");

    for size in [100, 1_000, 10_000].iter() {
        let content = generate_html(*size);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(BenchmarkId::new("onego", size), &content, |b, html| {
            b.iter(|| {
                let queries = &[Query::all(QUERY, Save::all()).build()];
                let arena = parse(&html, queries);
                let root = &arena[0];
                let indices = root[QUERY].iter().unwrap();

                //assert_eq!(iterator.count(), MAX_ELEMENT_LEN);

                for element in indices.map(|i| &arena[*i]) {
                    black_box(&element.attributes);
                    black_box(&element.inner_html);
                    black_box(&element.text_content);
                }
            })
        });

        // group.bench_with_input(
        //     BenchmarkId::new("onego_no_store", size),
        //     &content,
        //     |b, html| {
        //         b.iter(|| {
        //             let queries = &[Query::all(black_box(QUERY), Save::none()).build()];
        //             let res = black_box(fake_parse(html, queries));
        //         })
        //     },
        // );

        group.bench_with_input(BenchmarkId::new("tl", size), &content, |b, html| {
            b.iter(|| {
                let dom = tl::parse(html, ParserOptions::default()).unwrap();
                let parser = dom.parser();
                let query = dom.query_selector(QUERY).unwrap();

                for node_handle in query {
                    if let Some(node) = node_handle.get(parser) {
                        let attributes = node.as_tag().unwrap().attributes();
                        black_box(attributes.get("href"));
                        black_box(node.inner_html(parser));
                        black_box(node.inner_text(parser));
                    }
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("scraper", size), &content, |b, html| {
            b.iter(|| {
                let document = Html::parse_document(html);
                let selector = Selector::parse(QUERY).unwrap();

                for element in document.select(&selector) {
                    black_box(element.attr("href"));
                    black_box(element.inner_html());
                    black_box(element.text().collect::<Vec<&str>>());
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("lexbor", size), &content, |b, html| {
            b.iter(|| {
                let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
                let nodes = doc.select(QUERY);

                for node in nodes.iter() {
                    // TODO: I need to add attributes and innerhtml for lexbor
                    black_box(node.text_content());
                    black_box(node.inner_html());
                    black_box(node.attributes());
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("lol_html", size), &content, |b, html| {
            b.iter(|| {
                let mut rewriter = HtmlRewriter::new(
                    Settings {
                        element_content_handlers: vec![element!(QUERY, |el| {
                            black_box(el.get_attribute("href"));
                            Ok(())
                        })],
                        ..Settings::default()
                    },
                    |_: &[u8]| {},
                );
                rewriter.write(html.as_bytes()).unwrap();
                rewriter.end().unwrap();
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_comparison);
criterion_main!(benches);
