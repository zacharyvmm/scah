//! Validate queries at compile time with `query!`.
//!
//! The macro checks every selector while your crate compiles, so an
//! unsupported selector is a build error rather than a runtime one, and it
//! builds the query without heap allocation.
//!
//! ```sh
//! cargo run --example compile_time
//! ```

use scah::{parse, query};

const HTML: &str = r#"
<article>
  <h1>Streaming HTML extraction</h1>
  <p>Select what you need. <a href="/docs">Read the docs</a>.</p>
  <p>Skip the DOM. <a href="/benchmarks">See the benchmarks</a>.</p>
</article>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queries = [query! {
        all("article", Save::none()) => {
            first("> h1", Save::only_text()),
            all("a[href]", Save::only_text()),
        }
    }];
    let store = parse(HTML, &queries)?;

    for article in store.get("article").into_iter().flatten() {
        if let Some(title) = article.get("> h1").and_then(|mut h1| h1.next()) {
            println!("# {}", title.text().unwrap_or_default());
        }
        for link in article.get("a[href]").into_iter().flatten() {
            println!(
                "- {} ({})",
                link.text().unwrap_or_default(),
                link.attribute("href").unwrap_or_default()
            );
        }
    }
    Ok(())
}
