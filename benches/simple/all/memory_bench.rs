use gungraun::{LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use std::hint::black_box;

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
    let queries = &[Query::all(QUERY, Save::all()).build()];

    let store = parse(&html, queries);

    //assert_eq!(iterator.count(), MAX_ELEMENT_LEN);
    let root_element = &store.elements[0];

    for element in root_element
        .select(QUERY)
        .iter()
        .unwrap()
        .map(|i| &store.elements[*i])
    {
        black_box(&element.inner_html);
        black_box(store.attributes(&element));
        let text_content = if let Some(content) = store.text_content(&element) {
            content.join(" ")
        } else {
            String::from("")
        };
        black_box(text_content);
    }
}

use scraper::{Html, Selector};
#[library_benchmark]
#[bench::scraper(setup_html())]
fn bench_scraper(html: String) {
    let document = Html::parse_document(&html);

    let selector = Selector::parse(QUERY).unwrap();
    let iterator = document.select(&selector);

    // assert_eq!(iterator.count(), MAX_ELEMENT_LEN);

    for element in iterator {
        black_box(element.inner_html());
        black_box(element.attr("href"));
        black_box(element.text());
    }
}

use tl::ParserOptions;
#[library_benchmark]
#[bench::tl(setup_html())]
fn bench_tl(html: String) {
    let dom = tl::parse(&html, ParserOptions::default()).unwrap();
    let parser = dom.parser();

    // tl doesn't work with complex queries `div.article a` returns a count of 0 while `a` works fine
    let query = dom.query_selector(QUERY).unwrap();
    //assert_eq!(query.count(), MAX_ELEMENT_LEN);

    for node_handle in query {
        if let Some(node) = node_handle.get(parser) {
            let attributes = node.as_tag().unwrap().attributes();
            black_box(attributes.get("href"));
            black_box(node.inner_html(parser));
            black_box(node.inner_text(parser));
        }
    }
}

use lexbor_css::HtmlDocument;
#[library_benchmark]
#[bench::lexbor(setup_html())]
fn bench_lexbor(html: String) {
    let doc = HtmlDocument::new(html.as_str()).expect("Failed to parse HTML");
    let nodes = doc.select(QUERY);
    let iterator = nodes.iter();

    //assert_eq!(iterator.count(), MAX_ELEMENT_LEN);

    for node in iterator {
        black_box(node.text_content());
        black_box(node.inner_html());
        black_box(node.attributes());
    }
}

use lol_html::{HtmlRewriter, Settings, element};
#[library_benchmark]
#[bench::lol_html(setup_html())]
fn bench_lol_html(html: String) {
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
}

library_benchmark_group!(
    name = comparison_group;
    benchmarks = bench_onego, bench_tl, bench_scraper, bench_lexbor, bench_lol_html
);

main!(library_benchmark_groups = comparison_group);
