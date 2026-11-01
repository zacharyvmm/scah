//! Print every link in an HTML document.
//!
//! ```sh
//! cargo run --example links                 # a built-in sample
//! cargo run --example links -- page.html    # a file of your own
//! ```

use scah::{Query, Save, parse};

const SAMPLE: &str = r#"
<nav>
  <a href="/">Home</a>
  <a href="/docs">Docs &amp; guides</a>
  <a>No href, not selected</a>
  <a href="https://github.com/zacharyvmm/scah">Source</a>
</nav>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(path)?,
        None => SAMPLE.to_owned(),
    };

    let queries = [Query::all("a[href]", Save::only_text())?.build()];
    let store = parse(&html, &queries)?;

    for link in store.get("a[href]").into_iter().flatten() {
        let href = link.attribute("href").unwrap_or_default();
        let text = link.text().unwrap_or_default();
        println!("{href}\t{text}");
    }
    Ok(())
}
