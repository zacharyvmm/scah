//! Workloads shared by the columnar store speed bench and memory report.

#[path = "../support/mod.rs"]
#[allow(dead_code)]
mod support;

use scah::{ColumnarStore, Element, ElementRef, Query, Save, Store};
use std::hint::black_box;
use std::path::PathBuf;

pub const SPEC_HTML_FILE: &str = "html.spec.whatwg.org.html";

/// Inner HTML, normalized text, and attributes: what the comparison benches
/// in `simple/` and `nested/` save.
pub const CONTENT_SAVE: Save = Save {
    inner_html: true,
    raw_text: false,
    text: true,
    attributes: true,
};

pub struct Workload {
    pub name: &'static str,
    pub html: &'static str,
    pub queries: Vec<Query<'static>>,
    pub root: &'static str,
    pub nested: &'static [&'static str],
}

fn links_html(count: usize) -> String {
    let mut html = String::with_capacity(count * 100);
    html.push_str("<html><body><div id='content'>");
    for i in 0..count {
        html.push_str(&format!(
            r#"<div class="article"><a href="/post/{i}"><b>Post</b> &lt;{i}&gt;</a></div>"#
        ));
    }
    html.push_str("</div></body></html>");
    html
}

/// The WHATWG spec page from `SCAH_BENCH_DATA_DIR` if set, otherwise from
/// `benches/bench_data`. `None` (with a note) when the file is missing.
fn spec_html() -> Option<String> {
    let path = std::env::var_os("SCAH_BENCH_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| support::bench_data_path(""))
        .join(SPEC_HTML_FILE);
    match std::fs::read_to_string(&path) {
        Ok(html) => Some(html),
        Err(err) => {
            eprintln!(
                "skipping WHATWG spec workloads: cannot read {}: {err}",
                path.display()
            );
            None
        }
    }
}

fn leak(html: String) -> &'static str {
    Box::leak(html.into_boxed_str())
}

fn links(name: &'static str, html: &'static str, query: Query<'static>) -> Workload {
    Workload {
        name,
        html,
        queries: vec![query],
        root: "a",
        nested: &[],
    }
}

pub fn workloads() -> Vec<Workload> {
    let links_10k = leak(links_html(10_000));
    let catalog_10k = leak(support::generate_product_catalog_html(10_000));
    let mut workloads = vec![
        links(
            "links_10k_name_only",
            links_10k,
            Query::all("a", Save::name_only()).unwrap().build(),
        ),
        links(
            "links_10k_save_none",
            links_10k,
            Query::all("a", Save::none()).unwrap().build(),
        ),
        links(
            "links_10k_content",
            links_10k,
            Query::all("a", CONTENT_SAVE).unwrap().build(),
        ),
        links(
            "links_10k_first",
            links_10k,
            Query::first("a", CONTENT_SAVE).unwrap().build(),
        ),
        Workload {
            name: "catalog_10k_nested",
            html: catalog_10k,
            queries: vec![
                Query::all(support::PRODUCT_SELECTOR, CONTENT_SAVE)
                    .unwrap()
                    .then(|product| {
                        Ok([
                            product.first(support::PRODUCT_TITLE_SELECTOR, CONTENT_SAVE)?,
                            product.first(support::PRODUCT_RATING_SELECTOR, CONTENT_SAVE)?,
                            product.first(support::PRODUCT_DESCRIPTION_SELECTOR, CONTENT_SAVE)?,
                        ])
                    })
                    .unwrap()
                    .build(),
            ],
            root: support::PRODUCT_SELECTOR,
            nested: &[
                support::PRODUCT_TITLE_SELECTOR,
                support::PRODUCT_RATING_SELECTOR,
                support::PRODUCT_DESCRIPTION_SELECTOR,
            ],
        },
    ];
    if let Some(spec) = spec_html() {
        let spec = leak(spec);
        workloads.push(links(
            "spec_links_save_none",
            spec,
            Query::all("a", Save::none()).unwrap().build(),
        ));
        workloads.push(links(
            "spec_links_content",
            spec,
            Query::all("a", CONTENT_SAVE).unwrap().build(),
        ));
    }
    workloads
}

#[inline(always)]
/// Consume a value the way a caller would: its length and first byte. This
/// forces the string to be materialized (pointer and length) without a
/// `black_box` per field, which would act as a memory clobber and force both
/// stores to reload their state between fields.
fn len(value: Option<&str>) -> usize {
    value.map_or(0, |value| {
        value.len() + usize::from(value.as_bytes().first().copied().unwrap_or(0))
    })
}

#[inline(always)]
fn read_element(store: &Store<'_, '_>, element: &Element<'_>) -> usize {
    len(Some(element.name))
        + len(element.class)
        + len(element.id)
        + len(element.inner_html)
        + len(element.attribute(store, "href"))
        + element.attributes(store).map_or(0, <[_]>::len)
        + len(element.raw_text(store))
        + len(element.text(store))
}

#[inline(always)]
fn read_ref(element: ElementRef<'_, '_, '_>) -> usize {
    len(Some(element.name()))
        + len(element.class_name())
        + len(element.html_id())
        + len(element.inner_html())
        + len(element.attribute("href"))
        + element.attributes().map_or(0, <[_]>::len)
        + len(element.raw_text())
        + len(element.text())
}

/// Read every field of every root match and every nested match.
pub fn read_store(store: &Store<'_, '_>, workload: &Workload) -> usize {
    let mut total = 0;
    for element in store.get(workload.root).into_iter().flatten() {
        total += read_element(store, element);
        for selector in workload.nested {
            for child in element.get(store, selector).into_iter().flatten() {
                total += read_element(store, child);
            }
        }
    }
    total
}

/// Read every field of every root match and every nested match.
pub fn read_columnar(store: &ColumnarStore<'_, '_>, workload: &Workload) -> usize {
    let mut total = 0;
    for element in store.get(workload.root).into_iter().flatten() {
        total += read_ref(element);
        for selector in workload.nested {
            for child in element.get(selector).into_iter().flatten() {
                total += read_ref(child);
            }
        }
    }
    total
}

/// Walk the result lists (`get` and nested `get`) without reading fields.
pub fn traverse_store(store: &Store<'_, '_>, workload: &Workload) -> usize {
    let mut total = 0;
    for element in store.get(workload.root).into_iter().flatten() {
        total += black_box(element) as *const _ as usize & 1;
        for selector in workload.nested {
            total += element.get(store, selector).map_or(0, Iterator::count);
        }
    }
    total
}

/// Walk the result lists (`get` and nested `get`) without reading fields.
pub fn traverse_columnar(store: &ColumnarStore<'_, '_>, workload: &Workload) -> usize {
    let mut total = 0;
    for element in store.get(workload.root).into_iter().flatten() {
        total += black_box(element.id()) as usize & 1;
        for selector in workload.nested {
            total += element.get(selector).map_or(0, Iterator::count);
        }
    }
    total
}
