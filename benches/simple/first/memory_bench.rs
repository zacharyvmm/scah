use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use lol_html::errors::RewritingError;
use std::error::Error;
use std::fmt;
use std::hint::black_box;

#[derive(Debug)]
struct StopParsing;

impl fmt::Display for StopParsing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Stop parsing")
    }
}

impl Error for StopParsing {}

const MAX_ELEMENT_LEN: usize = 1000;
const QUERY: &str = black_box("a");

fn setup_html() -> String {
    let mut html = String::new();
    html.push_str("<html><body><div id='content'>");
    for i in 0..MAX_ELEMENT_LEN {
        html.push_str(&format!(
            r#"<div class="article"><a href="/post/{}">Post {}</a></div>"#,
            i, i
        ));
    }
    html.push_str("</div></body></html>");
    html
}

use onego::{Query, Save, parse};
#[library_benchmark]
#[bench::onego(setup_html())]
fn bench_onego(html: String) {
    let queries = &[Query::first(QUERY, Save::all()).build()];

    let store = parse(&html, queries);
    let root = &store.arena[0];

    let element_index = root[QUERY].value().unwrap();
    let element = &store.arena[element_index];

    black_box(&element.attributes);
    black_box(&element.inner_html);
    black_box(store.text_content(&element));
}

use scraper::{Html, Selector};
#[library_benchmark]
#[bench::scraper(setup_html())]
fn bench_scraper(html: String) {
    let document = Html::parse_document(&html);

    let selector = Selector::parse(QUERY).unwrap();

    let element = document.select(&selector).next().unwrap();
    black_box(element.attr("href"));
    black_box(element.inner_html());
    black_box(element.text().collect::<Vec<&str>>());
}

use tl::ParserOptions;
#[library_benchmark]
#[bench::tl(setup_html())]
fn bench_tl(html: String) {
    let dom = tl::parse(&html, ParserOptions::default()).unwrap();
    let parser = dom.parser();

    let node_handle = dom.query_selector(QUERY).unwrap().next().unwrap();

    if let Some(node) = node_handle.get(parser) {
        let attributes = node.as_tag().unwrap().attributes();
        black_box(attributes.get("href"));
        black_box(node.inner_html(parser));
        black_box(node.inner_text(parser));
    }
}

use lexbor_css::HtmlDocument;
#[library_benchmark]
#[bench::lexbor(setup_html())]
fn bench_lexbor(html: String) {
    let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
    let nodes = doc.select(QUERY);

    let node = nodes.iter().next().unwrap();
    black_box(node.text_content());
    black_box(node.inner_html());
    black_box(node.attributes());
}

use lol_html::{HtmlRewriter, Settings, element};
#[library_benchmark]
#[bench::lol_html(setup_html())]
fn bench_lol_html(html: String) {
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
            rewriter.end().unwrap();
        }
        Err(e) => panic!("Unexpected rewriting error: {}", e),
    }
}

library_benchmark_group!(
    name = comparison_group;
    benchmarks = bench_onego, bench_tl, bench_scraper, bench_lexbor, bench_lol_html
);

main!(library_benchmark_groups = comparison_group);
