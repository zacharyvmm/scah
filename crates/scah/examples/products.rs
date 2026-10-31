//! Extract structured records with nested queries.
//!
//! Each child query runs only inside a match of its parent, so every product
//! keeps its own name, price, and tags.
//!
//! ```sh
//! cargo run --example products
//! ```

use scah::{Query, Save, parse};

const HTML: &str = r#"
<main>
  <article class="product" data-sku="L-100">
    <h2>Desk lamp</h2>
    <span class="price">$24.00</span>
    <ul class="tags"><li>lighting</li><li>office</li></ul>
  </article>
  <article class="product" data-sku="C-200">
    <h2>Office chair</h2>
    <span class="price">$189.00</span>
    <ul class="tags"><li>seating</li></ul>
  </article>
</main>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queries = [Query::all("article.product", Save::none())?
        .then(|product| {
            Ok([
                product.first("> h2", Save::only_text())?,
                product.first(".price", Save::only_text())?,
                product.all(".tags > li", Save::only_text())?,
            ])
        })?
        .build()];
    let store = parse(HTML, &queries)?;

    for product in store.get("article.product").into_iter().flatten() {
        let sku = product.attribute("data-sku").unwrap_or_default();
        let first_text = |selector| {
            product
                .get(selector)
                .and_then(|mut matches| matches.next())
                .and_then(|element| element.text())
                .unwrap_or_default()
        };
        let tags: Vec<_> = product
            .get(".tags > li")
            .into_iter()
            .flatten()
            .filter_map(|tag| tag.text())
            .collect();

        println!(
            "{sku}: {} at {} [{}]",
            first_text("> h2"),
            first_text(".price"),
            tags.join(", ")
        );
    }
    Ok(())
}
