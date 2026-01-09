use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

const MAX_ELEMENT_LEN:usize = 1000;
const QUERY:&str = black_box("a");

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

    let res = parse(&html, queries);

    let element = res[QUERY].value().unwrap();
    black_box(&element.attributes);
    black_box(&element.inner_html);
    black_box(&element.text_content);
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

library_benchmark_group!(
    name = comparison_group;
    benchmarks = bench_onego, bench_tl, bench_scraper, bench_lexbor
);

main!(library_benchmark_groups = comparison_group);
