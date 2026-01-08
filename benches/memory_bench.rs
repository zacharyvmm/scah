use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

fn setup_html() -> String {
    let mut html = String::new();
    html.push_str("<html><body><div id='content'>");
    for i in 0..1000 {
        html.push_str(&format!(
            r#"<div class="article"><a href="/post/{}">Post {}</a></div>"#,
            i, i
        ));
    }
    html.push_str("</div></body></html>");
    html
}

use onego::{Query, Save, fake_parse, parse};

#[library_benchmark]
#[bench::onego(setup_html())]
fn bench_onego(html: String) {
    let queries = &[Query::all(black_box("div.article a"), Save::all()).build()];

    let res = parse(&html, queries);

    for element in res["div.article a"].iter().unwrap() {
        black_box(&element.attributes);
        black_box(&element.inner_html);
        black_box(&element.text_content);
    }
}

#[library_benchmark]
#[bench::onego_no_store(setup_html())]
fn bench_onego_no_store(html: String) {
    let queries = &[Query::all(black_box("div.article a"), Save::none()).build()];

    let res = black_box(fake_parse(&html, queries));
}

#[library_benchmark]
#[bench::onego_first_element(setup_html())]
fn bench_onego_first_element(html: String) {
    let queries = &[Query::first(black_box("div.article a"), Save::all()).build()];

    let res = parse(&html, queries);

    let element = res["div.article a"].value().unwrap();
    black_box(&element.attributes);
    black_box(&element.inner_html);
    black_box(&element.text_content);
}

use scraper::{Html, Selector};
#[library_benchmark]
#[bench::scraper(setup_html())]
fn bench_scraper(html: String) {
    let document = Html::parse_document(&html);

    let selector = Selector::parse(black_box("div.article a")).unwrap();

    for element in document.select(&selector) {
        black_box(element.inner_html());
        black_box(element.attr("href"));
        black_box(element.text());
    }
}

#[library_benchmark]
#[bench::scraper_first_element(setup_html())]
fn bench_scraper_first_element(html: String) {
    let document = Html::parse_document(&html);

    let selector = Selector::parse(black_box("div.article a")).unwrap();

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

    let query = dom.query_selector(black_box("div.article a")).unwrap();

    for node_handle in query {
        if let Some(node) = node_handle.get(parser) {
            let attributes = node.as_tag().unwrap().attributes();
            black_box(attributes.get("href"));
            black_box(node.inner_html(parser));
            black_box(node.inner_text(parser));
        }
    }
}

#[library_benchmark]
#[bench::tl_first_element(setup_html())]
fn bench_tl_first_element(html: String) {
    let dom = tl::parse(&html, ParserOptions::default()).unwrap();
    let parser = dom.parser();

    let node_handle = dom.query_selector("div.article a").unwrap().next().unwrap();

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
    let nodes = doc.select(black_box("div.article a"));

    for node in nodes.iter() {
        black_box(node.text_content());
        black_box(node.inner_html());
        black_box(node.attributes());
    }
}

#[library_benchmark]
#[bench::lexbor_first_element(setup_html())]
fn bench_lexbor_first_element(html: String) {
    let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
    let nodes = doc.select(black_box("div.article a"));

    let node = nodes.iter().next().unwrap();
    black_box(node.text_content());
    black_box(node.inner_html());
    black_box(node.attributes());
}

library_benchmark_group!(
    name = comparison_group;
    benchmarks = /*bench_onego, bench_onego_no_store, bench_onego_first_element,*/ bench_tl, /*bench_tl_first_element, bench_scraper, bench_scraper_first_element, bench_lexbor, bench_lexbor_first_element*/
);

main!(library_benchmark_groups = comparison_group);
