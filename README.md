# scah (scan HTML)
World's fastest CSS Selector.

> CSS selectors meet streaming XML/HTML parsing. Filter StAX events and build targeted DOMs without loading the entire document.

[![Crates.io](https://img.shields.io/crates/v/scah)](https://crates.io/crates/scah)
[![npm](https://img.shields.io/npm/v/%40zacharymm%2Fscah)](https://www.npmjs.com/package/@zacharymm/scah)
[![PyPI](https://img.shields.io/pypi/v/scah)](https://pypi.org/project/scah/)

## What is scah?

**scah** is a high-performance parsing library that bridges the gap between SAX/StAX streaming efficiency and DOM convenience. Instead of loading an entire document into memory or manually tracking parser state, you declare what you want with **CSS selectors**; the library handles the streaming complexity and builds a targeted DOM containing only your selections.

- **Streaming core**: Built on StAX; constant memory regardless of document size
- **Familiar API**: CSS selectors (including combinators like `>`, ` `, `+` (coming soon), `~` (coming soon))
- **Multi-language**: Rust core with Python and TypeScript/JavaScript bindings
- **Composable queries**: Chain selections and nest them with closures for **structured querying**; not only more efficient than flat filtering, but a fundamentally better pattern for extracting hierarchical data relationships

## Quick Start

### Rust

```toml
# Cargo.toml
[dependencies]
scah = "0.0.13"
```

#### Basic usage
```rust
use scah::{Query, Save, parse};

let html = r#"<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>"#;

let queries = &[Query::all("a[href]", Save::all()).build()];
let store = parse(html, queries);

for a in store.get("a[href]").unwrap() {
    let href = a.attribute(&store, "href").unwrap();
    let text = a.text_content(&store).unwrap_or_default();
    println!("{text}: {href}");
}
// Output:
//   One: /one
//   Two: /two
```

#### Structured querying with `.then()`

Instead of flat filtering, nest queries with closures. Child queries only run within the context of their parent match:

```rust
use scah::{Query, Save, parse};

let query = Query::all("main > section", Save::all())
    .then(|section| [
        section.all("> a[href]", Save::all()),
        section.all("div a", Save::all()),
    ])
    .build();

let store = parse(html, &[query]);

// Access nested results through parent elements
for section in store.get("main > section").unwrap() {
    println!("Section: {}", section.inner_html.unwrap_or(""));

    if let Some(links) = section.get(&store, "> a[href]") {
        for link in links {
            println!("  Direct link: {}", link.attribute(&store, "href").unwrap());
        }
    }
}
```

If selectors come from user input, prefer `Query::try_all(...)` / `Query::try_first(...)` in Rust so malformed selectors return `SelectorParseError` instead of panicking.

#### `Save` options

Control what data is captured per selector:

| Constructor | `inner_html` | `text_content` | Use case |
|-------------|:---:|:---:|----------|
| `Save::all()` | Yes | Yes | Full extraction |
| `Save::only_inner_html()` | Yes | No | Raw markup only |
| `Save::only_text_content()` | No | Yes | Lightweight text scraping |
| `Save::none()` | No | No | Structure-only (attributes still saved) |

#### Supported CSS selector syntax

| Syntax | Example | Status |
|--------|---------|--------|
| Tag name | `a`, `div` | Working |
| ID | `#my-id` | Working |
| Class | `.my-class` | Working |
| Descendant | `main section a` | Working |
| Child | `main > section` | Working |
| Attribute presence | `a[href]` | Working |
| Attribute exact | `a[href="url"]` | Working |
| Attribute prefix | `a[href^="https"]` | Working |
| Attribute suffix | `a[href$=".com"]` | Working |
| Attribute substring | `a[href*="example"]` | Working |
| Adjacent sibling | `h1 + p` | Coming soon |
| General sibling | `h1 ~ p` | Coming soon |

> Full API documentation: [docs.rs/scah](https://docs.rs/scah)

#### Benchmarks
![Criterion BenchMarks](https://raw.githubusercontent.com/zacharyvmm/scah/main/benches/images/criterion_bench.png)

### Python
```bash
pip install -U scah
```
```python
from scah import Query, Save, parse 

query = Query.all("main > section", Save.all())
    .then(lambda section: [
        section.all("> a[href]", Save.all()),
        section.all("div a", Save.all()),
    ])
    .build()

store = parse(html, [query])
```

#### Benchmark's
##### Real Html BenchMark ([html.spec.whatwg.org](https://html.spec.whatwg.org/)) (select all `a` tags):
![WhatWg Html Spec BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/python/benches/images/whatwg.png)

##### Synthetic Html BenchMark (select all `a` tags):
![Synthetic Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/python/benches/images/synthetic.png)

### Typescript / Javascript
```bash
npm install scah@npm:@zacharymm/scah
```

```ts
import { Query, parse } from 'scah';

const query = Query.all('main > section', { innerHtml: true, textContent: true })
  .then((p) => [
    p.all('> a[href]', { innerHtml: true, textContent: true }),
    p.all('div a', { innerHtml: true, textContent: true }),
  ])
  .build();

const store = parse(html, [query]);
```
