use criterion::{
    criterion_group, criterion_main, 
    Criterion, BenchmarkId, Throughput
};
use std::hint::black_box;
use scraper::{Html, Selector};
use tl::ParserOptions;
use lexbor_rust;
use onego::{parse, Save, Selection, SelectionKind, SelectionPart};

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
    let mut group = c.benchmark_group("text_extraction_comparison");

    for size in [100, 1_000, 10_000].iter() {
        let content = generate_html(*size);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(BenchmarkId::new("onego", size), &content, |b, html| {
            b.iter(|| {
                let queries = vec![Selection::new(SelectionPart::new(
                    black_box("div.article a"), 
                    SelectionKind::All(Save { inner_html: false, text_content: true })
                ))];
                let res = parse(html, &queries);
                black_box(res);
            })
        });

        group.bench_with_input(BenchmarkId::new("tl", size), &content, |b, html| {
            b.iter(|| {
                let dom = tl::parse(html, ParserOptions::default()).unwrap();
                let parser = dom.parser();
                let query = dom.query_selector(black_box("div.article a")).unwrap();
                
                for node_handle in query {
                    if let Some(node) = node_handle.get(parser) {
                        black_box(node.inner_text(parser));
                    }
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("scraper", size), &content, |b, html| {
            b.iter(|| {
                let document = Html::parse_document(html);
                let selector = Selector::parse(black_box("div.article a")).unwrap();
                
                for element in document.select(&selector) {
                    let text: String = element.text().collect();
                    black_box(text);
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("lexbor", size), &content, |b, html| {
            b.iter(|| {
                let _ = lexbor_rust::parse_and_select(html.as_str(), black_box("div.article a"));
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_comparison);
criterion_main!(benches);
