# scah

**Fast HTML extraction with CSS selectors. One pass, no DOM.**

[![Crates.io](https://img.shields.io/crates/v/scah)](https://crates.io/crates/scah)
[![npm](https://img.shields.io/npm/v/%40zacharymm%2Fscah)](https://www.npmjs.com/package/@zacharymm/scah)
[![PyPI](https://img.shields.io/pypi/v/scah)](https://pypi.org/project/scah/)
[![docs.rs](https://img.shields.io/docsrs/scah)](https://docs.rs/scah)
[![Tests](https://github.com/zacharyvmm/scah/actions/workflows/tests.yml/badge.svg)](https://github.com/zacharyvmm/scah/actions/workflows/tests.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/zacharyvmm/scah/blob/main/LICENSE)

scah (*scan HTML*) pulls the elements you select out of an HTML document in a
single forward pass. You write CSS selectors; scah compiles them, streams the
document through its parser once, and keeps only the matches. There is no
tree to build or walk, so it is fast on large pages and cheap on memory.

It has a Rust core and bindings for Python, JavaScript/TypeScript, Java,
.NET, and C.

```python
from scah import Query, Save, parse

store = parse(html, [Query.all("a[href]", Save.only_text()).build()])
for link in store.get("a[href]"):
    print(link.text, link.get_attribute("href"))
```

## Why scah

- **Fast.** Finding every link in the 15 MB WHATWG HTML specification takes
  scah about 1.3× less time than lol_html and 7× less than scraper in Rust,
  4× less than selectolax and 50× less than BeautifulSoup in Python, 8× less
  than linkedom and 13× less than cheerio in JavaScript, 3.5× less than jsoup
  in Java, and 15× less than AngleSharp in .NET. See [Benchmarks](#benchmarks).
- **Only what you ask for.** Each query says what to keep: attributes, inner
  HTML, source text, or normalized text. Nothing else is stored.
- **Structured results.** Nest queries with `.then()` to extract records,
  such as each product with its own name and price, in the same single pass.
- **Correct or an error.** A selector scah can't evaluate in one pass, such as
  `:has()`, is rejected when the query is built. It is never run as a
  narrower selector that silently returns less.
- **Built for real-world HTML.** Implied end tags, misnested tags, unquoted
  attributes, and truncated documents are handled with HTML's own recovery
  rules.
- **Early exit.** When every query is a `first()`, the parse stops as soon as
  each one has its match.
- **Columnar output.** Export matches as a zero-copy Apache Arrow table for
  polars, pyarrow, or DuckDB, or scrape a whole column of HTML pages inside
  polars with `scah.polars.parse`.

## Install

| Language | Package | Command |
| -------- | ------- | ------- |
| Rust | [`scah`](https://crates.io/crates/scah) | `cargo add scah` |
| Python 3.10+ | [`scah`](https://pypi.org/project/scah/) | `pip install scah` |
| JavaScript / TypeScript | [`@zacharymm/scah`](https://www.npmjs.com/package/@zacharymm/scah) | `npm install @zacharymm/scah` |
| Java 11+ | [`crates/bindings/scah-java`](https://github.com/zacharyvmm/scah/tree/main/crates/bindings/scah-java) | See its README |
| .NET 8+ | [`crates/bindings/scah-csharp`](https://github.com/zacharyvmm/scah/tree/main/crates/bindings/scah-csharp) | See its README |
| C / C++ | [`crates/bindings/scah-c`](https://github.com/zacharyvmm/scah/tree/main/crates/bindings/scah-c) | `cargo build --release -p scah-c` |

```toml
[dependencies]
scah = "0.1.0"
```

Prebuilt Python wheels and npm binaries cover Linux with glibc (x86-64, ARM64), macOS
(Intel, Apple silicon), and Windows (x86-64).

## Quick start

### Rust

```rust
use scah::{Query, Save, parse};

let html = r#"<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>"#;

let queries = [Query::all("a[href]", Save::only_text())?.build()];
let store = parse(html, &queries)?;

for a in store.get("a[href]").unwrap() {
    println!("{}: {}", a.text().unwrap(), a.attribute("href").unwrap());
}
// One: /one
// Two: /two
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Python

```python
from scah import Query, Save, parse

html = '<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>'

store = parse(html, [Query.all("a[href]", Save.only_text()).build()])
for a in store.get("a[href]"):
    print(f"{a.text}: {a.get_attribute('href')}")
```

### JavaScript / TypeScript

```ts
import { Query, parse } from '@zacharymm/scah'

const html = '<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>'

const store = parse(html, [Query.all('a[href]', { text: true }).build()])
for (const a of store.get('a[href]') ?? []) {
  console.log(`${a.text}: ${a.getAttribute('href')}`)
}
```

### Java, .NET, C, and C++

The Java and .NET bindings, and the C ABI they are built on, expose the same
queries and results. Their READMEs have install instructions and complete
examples:
[Java](https://github.com/zacharyvmm/scah/blob/main/crates/bindings/scah-java/README.md),
[.NET](https://github.com/zacharyvmm/scah/blob/main/crates/bindings/scah-csharp/README.md),
[C and C++](https://github.com/zacharyvmm/scah/blob/main/crates/bindings/scah-c/README.md).

## Structured extraction

Child queries added with `.then()` run inside each match of their parent, so
every record keeps its own fields:

```rust
use scah::{Query, Save, parse};

let html = r#"
    <article class="product"><h2>Desk lamp</h2><span class="price">$24</span></article>
    <article class="product"><h2>Office chair</h2><span class="price">$189</span></article>
"#;

let queries = [Query::all("article.product", Save::none())?
    .then(|product| {
        Ok([
            product.first("> h2", Save::only_text())?,
            product.first(".price", Save::only_text())?,
        ])
    })?
    .build()];
let store = parse(html, &queries)?;

for product in store.get("article.product").unwrap() {
    let name = product.get("> h2").unwrap().next().unwrap();
    let price = product.get(".price").unwrap().next().unwrap();
    println!("{}: {}", name.text().unwrap(), price.text().unwrap());
}
// Desk lamp: $24
// Office chair: $189
# Ok::<(), Box<dyn std::error::Error>>(())
```

The same query in Python and JavaScript:

```python
query = (
    Query.all("article.product", Save.none())
    .then(lambda product: [
        product.first("> h2", Save.only_text()),
        product.first(".price", Save.only_text()),
    ])
    .build()
)
```

```ts
const query = Query.all('article.product')
  .then((product) => [product.first('> h2', { text: true }), product.first('.price', { text: true })])
  .build()
```

In Rust, the `query!` macro builds the same query at compile time, so an
unsupported selector is a build error and the query needs no allocation:

```rust
use scah::{parse, query};

let queries = [query! {
    all("article.product", Save::none()) => {
        first("> h2", Save::only_text()),
        first(".price", Save::only_text()),
    }
}];
# let store = parse("<article class='product'></article>", &queries).unwrap();
```

More runnable programs are in [`crates/scah/examples`](https://github.com/zacharyvmm/scah/tree/main/crates/scah/examples),
including reusing compiled queries across many documents.

## What to save

| Rust | Python | JavaScript | Saves |
| ---- | ------ | ---------- | ----- |
| `Save::all()` | `Save.all()` | `{ innerHtml: true, rawText: true, text: true }` | Inner HTML, raw text, and normalized text |
| `Save::only_text()` | `Save.only_text()` | `{ text: true }` | Normalized, readable text |
| `Save::only_raw_text()` | `Save.only_raw_text()` | `{ rawText: true }` | Source text, markup removed |
| `Save::only_inner_html()` | `Save.only_inner_html()` | `{ innerHtml: true }` | The markup inside the element |
| `Save::none()` | `Save.none()` | `{}` | No content |
| `Save::name_only()` | `Save.name_only()` | `{ attributes: false }` | No content and no attributes |

Every option except `name_only` also keeps the element's id, class, and
attributes. Normalized text decodes character references, skips `script`,
`style`, and `hidden` content, and adds line breaks for block elements; see
[Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md).

## Arrow export

For bulk work, export matches as an [Apache Arrow](https://arrow.apache.org/)
table instead of element objects. String columns point straight into the
parsed HTML and text buffers, so nothing is copied and no per-element objects
are created. Any library that reads the Arrow C Data Interface can import the
table:

```python
import polars as pl
from scah import Query, Save, parse

store = parse(html, [Query.all("a", Save(text=True)).build()])
df = pl.DataFrame(store.to_arrow("a", attributes=["href"]))
# columns: index, tag, inner_html, raw_text, text, href
```

Pass `parent=` to export a child section, for example
`store.to_arrow("h1", parent="div.product")`. Each row then carries a
`parent` column holding the parent's `index`, ready to join. On 100,000 links,
building a polars DataFrame this way takes about a tenth of the time it takes
through element objects. Reading values back one by one (`to_pylist()`) is no
faster than element objects, so use Arrow when the data stays columnar.
From C, `scah_store_export_arrow` fills the standard `ArrowSchema` and
`ArrowArray` structs.

To scrape a whole column of HTML pages in polars, see
[`scah.polars`](https://github.com/zacharyvmm/scah/blob/main/crates/bindings/scah-python/README.md#polars).

## Selectors

scah supports type, id, class, universal, and attribute selectors (`=`,
`^=`, `$=`, `*=`, with `i` and `s` flags); descendant, child, `+`, and `~`
combinators; selector lists; `:not()`, `:is()`, `:where()`; `:first-child`,
`:nth-child()` (including `of S`), `:first-of-type`, `:nth-of-type()`;
`:root`; and `:scope` in nested queries.

Selectors that depend on content after the element, such as `:has()`,
`:empty`, and `:last-child`, are rejected. See
[Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md)
for the full list.

## When to use something else

scah is built for extraction. Reach for a different tool if you need to:

- **Rewrite HTML** as it streams: use [lol_html](https://github.com/cloudflare/lol-html).
- **Navigate freely** after parsing (parents, siblings, arbitrary traversal),
  or use selectors that look ahead, such as `:has()`: use a DOM library such
  as scraper, selectolax, or cheerio.
- **Match a browser's tree exactly** on badly broken markup: scah applies
  HTML's common recovery rules but not the full HTML5 tree-construction
  algorithm (for example, it doesn't move misplaced table content).

## Benchmarks

### Rust

<!-- benchmarks:rust -->
| Library | WHATWG spec | Nested (all) | Nested (first) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: | ---: |
| **scah** | **19.5 ms** | **3.91 ms** | **2.15 µs** | **1.47 ms** | **887 ns** |
| lol_html | 25.6 ms (1.31×) | 8.08 ms (2.06×) | 2.63 µs (1.22×) | 2.20 ms (1.50×) | 623 ns (0.70×) |
| tl | 33.5 ms (1.72×) | 9.23 ms (2.36×) | 2.49 ms (1,158×) | 3.19 ms (2.17×) | 1.75 ms (1,974×) |
| lexbor | 60.6 ms (3.11×) | 9.84 ms (2.51×) | 5.72 ms (2,655×) | 4.69 ms (3.19×) | 3.41 ms (3,841×) |
| scraper | 136 ms (7.00×) | 20.8 ms (5.32×) | 13.1 ms (6,092×) | 13.4 ms (9.13×) | 11.8 ms (13,249×) |
| lxml | 319 ms (16.4×) | 57.5 ms (14.7×) | 41.4 ms (19,208×) | 23.3 ms (15.8×) | 16.7 ms (18,834×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-29; raw data and run details in [`benches/results/rust`](https://github.com/zacharyvmm/scah/tree/main/benches/results/rust).
<!-- /benchmarks:rust -->

### Python

<!-- benchmarks:python -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **Scah** | **43.3 ms** | **10.5 ms** | **3.18 ms** | **8.28 µs** |
| Selectolax | 183 ms (4.23×) | 70.5 ms (6.71×) | 7.66 ms (2.41×) | 3.13 ms (379×) |
| lxml | 341 ms (7.87×) | 312 ms (29.6×) | 21.4 ms (6.72×) | 6.34 ms (765×) |
| Parsel | 631 ms (14.6×) | 260 ms (24.8×) | 68.4 ms (21.5×) | 10.3 ms (1,249×) |
| Gazpacho | 1.53 s (35.3×) | — | 119 ms (37.3×) | 60.9 ms (7,361×) |
| BS4 (lxml) | 2.18 s (50.3×) | 818 ms (77.8×) | 100 ms (31.5×) | 80.0 ms (9,662×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-29; raw data and run details in [`benches/results/python`](https://github.com/zacharyvmm/scah/tree/main/benches/results/python).
<!-- /benchmarks:python -->

### JavaScript (Bun)

<!-- benchmarks:node -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **50.0 ms** | **24.4 ms** | **5.28 ms** | **63.5 µs** |
| linkedom | 407 ms (8.14×) | 44.8 ms (1.83×) | 37.0 ms (7.00×) | 88.1 ms (1,388×) |
| cheerio | 659 ms (13.2×) | 89.7 ms (3.67×) | 31.9 ms (6.03×) | 27.0 ms (425×) |
| node-html-parser | 691 ms (13.8×) | 118 ms (4.84×) | 157 ms (29.8×) | 27.7 ms (436×) |
| jsdom | — | 288 ms (11.8×) | 337 ms (63.8×) | 311 ms (4,901×) |
| happy-dom | — | — | 132 ms (24.9×) | 115 ms (1,811×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-29; raw data and run details in [`benches/results/node`](https://github.com/zacharyvmm/scah/tree/main/benches/results/node).
<!-- /benchmarks:node -->

### Java

<!-- benchmarks:java -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **71.0 ms** | **25.5 ms** | **6.74 ms** | **76.1 µs** |
| scah (JNI) | 90.7 ms (1.28×) | 38.2 ms (1.50×) | 9.97 ms (1.48×) | 81.2 µs (1.07×) |
| jsoup | 252 ms (3.54×) | 55.6 ms (2.18×) | 22.0 ms (3.26×) | 16.5 ms (216×) |
| jsoup (StreamParser) | 308 ms (4.34×) | 58.2 ms (2.28×) | 28.7 ms (4.25×) | 5.24 µs (0.07×) |
| Jericho | 383 ms (5.40×) | 75.6 ms (2.96×) | 22.7 ms (3.37×) | 2.91 µs (0.04×) |
| HtmlCleaner | 1.36 s (19.1×) | 225 ms (8.80×) | 98.3 ms (14.6×) | 66.2 ms (870×) |
| Jodd Jerry | 1.63 s (22.9×) | 70.2 ms (2.75×) | 45.8 ms (6.78×) | 44.9 ms (590×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-31; raw data and run details in [`benches/results/java`](https://github.com/zacharyvmm/scah/tree/main/benches/results/java).
<!-- /benchmarks:java -->

### C#

<!-- benchmarks:csharp -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **63.4 ms** | **20.1 ms** | **5.16 ms** | **56.6 µs** |
| AngleSharp | 935 ms (14.7×) | 176 ms (8.72×) | 75.0 ms (14.5×) | 62.1 ms (1,097×) |
| HtmlAgilityPack | 1.62 s (25.5×) | 325 ms (16.1×) | 132 ms (25.5×) | 96.9 ms (1,710×) |
| HtmlAgilityPack + Fizzler | 1.68 s (26.4×) | 263 ms (13.0×) | 135 ms (26.2×) | 97.2 ms (1,717×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-31; raw data and run details in [`benches/results/csharp`](https://github.com/zacharyvmm/scah/tree/main/benches/results/csharp).
<!-- /benchmarks:csharp -->

Each column is one scenario: every link in the WHATWG HTML specification;
nested `.then()` queries; and every match, or only the first, in a flat
document. See [`benches/`](https://github.com/zacharyvmm/scah/blob/main/benches/README.md)
for what each scenario measures and how to reproduce these numbers.

## Documentation

- [API reference (docs.rs)](https://docs.rs/scah)
- [Guides](https://github.com/zacharyvmm/scah/blob/main/docs/README.md):
  selectors, text extraction, and architecture
- [Changelog](https://github.com/zacharyvmm/scah/blob/main/CHANGELOG.md),
  including migration notes

## Contributing

Bug reports, selector edge cases, and benchmark comparisons are welcome. See
[CONTRIBUTING.md](https://github.com/zacharyvmm/scah/blob/main/CONTRIBUTING.md)
to get started, and [SECURITY.md](https://github.com/zacharyvmm/scah/blob/main/SECURITY.md)
to report a vulnerability.

## License

scah is available under the [MIT License](https://github.com/zacharyvmm/scah/blob/main/LICENSE).
Its HTML character reference table is derived from the WHATWG HTML Standard;
see [`THIRD_PARTY_LICENSES`](https://github.com/zacharyvmm/scah/blob/main/crates/scah/THIRD_PARTY_LICENSES/WHATWG-HTML.txt).
