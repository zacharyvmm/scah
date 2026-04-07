use std::fs;
use std::path::PathBuf;

#[allow(dead_code)]
pub const SPEED_BENCH_SIZES: [usize; 3] = [100, 1_000, 10_000];
#[allow(dead_code)]
pub const MEMORY_BENCH_SIZE: usize = 1_000;

pub const PRODUCT_SELECTOR: &str = "div.product";
pub const PRODUCT_TITLE_SELECTOR: &str = "> h1";
pub const PRODUCT_RATING_SELECTOR: &str = "> span.rating";
pub const PRODUCT_DESCRIPTION_SELECTOR: &str = "> p.description";

pub const PRODUCT_TITLE_GLOBAL_SELECTOR: &str = "div.product > h1";
pub const PRODUCT_RATING_GLOBAL_SELECTOR: &str = "div.product > span.rating";
pub const PRODUCT_DESCRIPTION_GLOBAL_SELECTOR: &str = "div.product > p.description";

pub fn generate_product_catalog_html(count: usize) -> String {
    let mut html = String::with_capacity(count * 180);
    html.push_str(r#"<html><body><section id="products">"#);

    for i in 1..=count {
        let rating = ((i - 1) % 5) + 1;
        html.push_str(&format!(
            r#"<div class="product"><h1>Product #{i}</h1><span class="rating">{rating}/5</span><p class="description">Description</p></div>"#
        ));
    }

    html.push_str("</section></body></html>");
    html
}

#[allow(dead_code)]
pub fn bench_data_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("bench_data")
        .join(filename)
}

#[allow(dead_code)]
pub fn load_bench_data(filename: &str) -> String {
    let path = bench_data_path(filename);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read bench data {}: {err}", path.display()))
}
