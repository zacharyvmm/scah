use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use lexbor_css::HtmlDocument;
use lol_html::errors::RewritingError;
use lol_html::{HtmlRewriter, Settings, element};
use onego::{Query, Save, parse};
use scraper::{Html, Selector};
use std::error::Error;
use std::fmt;
use std::hint::black_box;
use tl::ParserOptions;

#[derive(Debug)]
struct StopParsing;

impl fmt::Display for StopParsing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Stop parsing")
    }
}

impl Error for StopParsing {}

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
    let mut group = c.benchmark_group("simple_first_selection_comparison");

    for size in [100, 1_000, 10_000].iter() {
        let content = generate_html(*size);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(BenchmarkId::new("onego", size), &content, |b, html| {
            b.iter(|| {
                let queries = &[Query::first(black_box(QUERY), Save::all()).build()];

                let store = parse(&html, queries);

                //assert_eq!(iterator.count(), MAX_ELEMENT_LEN);
                let root_element = &store.elements[0];
                let element_index = root_element.select(QUERY)[0];
                let element = &store.elements[element_index];

                black_box(&element.inner_html);
                black_box(store.attributes(&element));
                black_box(store.text_content(&element).unwrap().join(" "));
            })
        });

        group.bench_with_input(BenchmarkId::new("tl", size), &content, |b, html| {
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
        });

        group.bench_with_input(BenchmarkId::new("scraper", size), &content, |b, html| {
            b.iter(|| {
                let document = Html::parse_document(html);
                let selector = Selector::parse(QUERY).unwrap();

                let element = document.select(&selector).next().unwrap();
                black_box(element.attr("href"));
                black_box(element.inner_html());
                black_box(element.text().collect::<Vec<&str>>());
            })
        });

        group.bench_with_input(BenchmarkId::new("lexbor", size), &content, |b, html| {
            b.iter(|| {
                let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
                let nodes = doc.select(QUERY);

                let node = nodes.iter().next().unwrap();
                // TODO: I need to add attributes and innerhtml for lexbor
                black_box(node.text_content());
                black_box(node.inner_html());
                black_box(node.attributes());
            })
        });

        group.bench_with_input(BenchmarkId::new("lol_html", size), &content, |b, html| {
            b.iter(|| {
                let mut rewriter = HtmlRewriter::new(
                    Settings {
                        element_content_handlers: vec![element!(QUERY, |el| {
                            black_box(el.get_attribute("href"));
                            Err(Box::new(StopParsing))
                        })],
                        ..Settings::default()
                    },
                    |_: &[u8]| {},
                );
                let res = rewriter.write(html.as_bytes());
                match res {
                    Err(RewritingError::ContentHandlerError(e)) => {
                        if e.downcast_ref::<StopParsing>().is_some() {
                            return;
                        }
                        panic!("Unexpected error: {}", e);
                    }
                    Ok(_) => {
                        // If we didn't find anything, we must call end() to finish
                        rewriter.end().unwrap();
                    }
                    Err(e) => panic!("Unexpected rewriting error: {}", e),
                }
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_comparison);
criterion_main!(benches);
