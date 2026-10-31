//! Compile queries once and reuse them across many documents.
//!
//! `parse` compiles its queries on every call. `Program::compile` does that
//! work once, and `parse_compiled` runs the result on each document.
//!
//! ```sh
//! cargo run --release --example many_documents
//! ```

use scah::{Program, Query, Save, parse_compiled};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queries = [
        Query::first("head > title", Save::only_text())?.build(),
        Query::all("a[href]", Save::none())?.build(),
    ];
    let program = Program::compile(&queries);

    let pages = (1..=5).map(|page| {
        format!(
            "<html><head><title>Page {page}</title></head>\
             <body>{}</body></html>",
            (0..page)
                .map(|link| format!("<a href='/page/{page}/{link}'>link</a>"))
                .collect::<String>()
        )
    });

    for html in pages {
        let store = parse_compiled(&html, &program)?;
        // Look results up by query position: 0 is the title, 1 the links.
        let title = store
            .query(0)
            .and_then(|mut titles| titles.next())
            .and_then(|title| title.text())
            .unwrap_or_default();
        let links = store.query(1).map_or(0, |links| links.len());
        println!("{title}: {links} links");
    }
    Ok(())
}
