# scah (scan HTML)

> CSS selectors meet streaming HTML parsing. Extract exactly what you select, without building a DOM.

[![Crates.io](https://img.shields.io/crates/v/scah)](https://crates.io/crates/scah)
[![npm](https://img.shields.io/npm/v/%40zacharymm%2Fscah)](https://www.npmjs.com/package/@zacharymm/scah)
[![PyPI](https://img.shields.io/pypi/v/scah)](https://pypi.org/project/scah/)
[![docs.rs](https://img.shields.io/docsrs/scah)](https://docs.rs/scah)
[![Tests](https://github.com/zacharyvmm/scah/actions/workflows/tests.yml/badge.svg)](https://github.com/zacharyvmm/scah/actions/workflows/tests.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/zacharyvmm/scah/blob/main/LICENSE)

scah sits between SAX/StAX streaming and a full DOM. You declare what you want
with CSS selectors. scah matches them in a single forward pass over the
document and stores only the matches, so you don't track parser state by hand
and you don't pay to build or walk a tree you don't need.

- **Single pass, no DOM**: selectors are evaluated while the document streams
  through the parser; only matched elements are kept.
- **Familiar API**: CSS selectors, including descendant, child, and sibling
  combinators, attribute matchers, `:is()`/`:not()`, and `:nth-child()`.
- **Structured queries**: nest selectors with `.then()` so child queries run
  only inside their parent match. This is faster than flat filtering and keeps
  hierarchical data together.
- **Rust core** with Python, JavaScript/TypeScript, and C bindings.

## Install

| Language | Package | Install |
| -------- | ------- | ------- |
| Rust | [`scah`](https://crates.io/crates/scah) | `cargo add scah` |
| Python | [`scah`](https://pypi.org/project/scah/) | `pip install scah` |
| JavaScript / TypeScript | [`@zacharymm/scah`](https://www.npmjs.com/package/@zacharymm/scah) | `npm install @zacharymm/scah` |
| C / C++ | [`crates/bindings/scah-c`](crates/bindings/scah-c) | `cargo build --release -p scah-c` |

```toml
# Cargo.toml
[dependencies]
scah = "0.1.0"
```

## Quick start

### Rust

```rust
use scah::{Query, Save, parse};

let html = r#"<ul><li><a href="/one">One</a></li><li><a href="/two">Two</a></li></ul>"#;

let queries = &[Query::all("a[href]", Save::all())
    .expect("valid selector")
    .build()];
let store = parse(html, queries).expect("parse succeeds");

for a in store.get("a[href]").unwrap() {
    let href = a.attribute(&store, "href").unwrap();
    let text = a.text(&store).unwrap_or_default();
    println!("{text}: {href}");
}
// One: /one
// Two: /two
```

#### Structured queries with `.then()`

Child queries only run within the context of their parent match:

```rust
use scah::{Query, Save, parse};

let html = r#"
    <main>
        <section><a href="/one">One</a><div><a href="/two">Two</a></div></section>
    </main>
"#;

let query = Query::all("main > section", Save::all())
    .expect("valid selector")
    .then(|section| {
        Ok([
            section.all("> a[href]", Save::all())?,
            section.all("div a", Save::all())?,
        ])
    })
    .expect("valid child selectors")
    .build();

let queries = [query];
let store = parse(html, &queries).expect("parse succeeds");

for section in store.get("main > section").unwrap() {
    if let Some(links) = section.get(&store, "> a[href]") {
        for link in links {
            println!("direct link: {}", link.attribute(&store, "href").unwrap());
        }
    }
}
```

`Query::all` and `Query::first` return `Result`, so selectors from user input
surface as `SelectorParseError` instead of panicking.

#### Compile-time queries with `query!`

For selectors known at compile time, the `query!` macro validates the selector
tree during compilation and emits a `StaticQuery` backed by inline arrays
instead of heap-allocated query storage:

```rust
use scah::{Save, parse, query};

let html = r#"
    <article>
        <h1>Title</h1>
        <a href="/one">One</a>
        <a href="/two">Two</a>
    </article>
"#;

let query = query! {
    all("article", Save::none()) => {
        first("h1", Save::only_text()),
        all("a[href]", Save::all()),
    }
};
let queries = [query];
let store = parse(html, &queries).expect("parse succeeds");
for article in store.get("article").unwrap() {
    assert_eq!(article.get(&store, "a[href]").unwrap().count(), 2);
}
```

#### Choosing what to save

| Constructor | `inner_html` | `raw_text` | `text` | Use case |
|-------------|:---:|:---:|:---:|----------|
| `Save::all()` | Yes | Yes | Yes | Full extraction |
| `Save::only_inner_html()` | Yes | No | No | Raw markup only |
| `Save::only_raw_text()` | No | Yes | No | Source-preserving text |
| `Save::only_text()` | No | No | Yes | Normalized text scraping |
| `Save::none()` | No | No | No | Structure only (attributes are still saved) |
| `Save::name_only()` | No | No | No | Element names only (attributes are not saved) |

Full API documentation is on [docs.rs/scah](https://docs.rs/scah).

### Python

```python
from scah import Query, Save, parse

html = """
<main>
  <section><a href="/one">One</a><div><a href="/two">Two</a></div></section>
</main>
"""

query = (
    Query.all("main > section", Save.none())
    .then(lambda section: [
        section.all("> a[href]", Save.only_text()),
        section.all("div a", Save.only_text()),
    ])
    .build()
)

store = parse(html, [query])
for section in store.get("main > section"):
    for link in section.get("> a[href]"):
        print(link.text, link.get_attribute("href"))  # One /one
```

### JavaScript / TypeScript

```ts
import { Query, parse } from '@zacharymm/scah'

const html = `
<main>
  <section><a href="/one">One</a><div><a href="/two">Two</a></div></section>
</main>`

const query = Query.all('main > section', { attributes: false })
  .then((section) => [
    section.all('> a[href]', { text: true }),
    section.all('div a', { text: true }),
  ])
  .build()

const store = parse(html, [query])
for (const section of store.get('main > section') ?? []) {
  for (const link of section.get('> a[href]')) {
    console.log(link.text, link.getAttribute('href')) // One /one
  }
}
```

Node takes plain option objects instead of `Save` constructors:

```ts
{
  innerHtml?: boolean
  rawText?: boolean
  text?: boolean
  attributes?: boolean
  textContent?: boolean // deprecated alias for text
}
```

### C / C++

The C ABI in [`crates/bindings/scah-c`](crates/bindings/scah-c) exposes the
same queries and results through opaque handles. See its
[README](crates/bindings/scah-c/README.md) for build and link instructions
and a complete example.

## Selector support

| Syntax | Example | Notes |
|--------|---------|-------|
| Tag name | `a`, `div` | |
| ID | `#my-id` | |
| Class | `.my-class` | |
| Universal | `*`, `*.card` | |
| Descendant | `main section a` | |
| Child | `main > section` | |
| Adjacent sibling | `h1 + p` | |
| General sibling | `h1 ~ p` | |
| Attribute presence | `a[href]` | |
| Attribute value | `[href="url"]`, `^=`, `$=`, `*=` | Exact, prefix, suffix, substring |
| Attribute value flags | `[data-kind="FOO" i]`, `[data-kind="FOO" S]` | ASCII case-insensitive or sensitive; the flag itself is case-insensitive |
| Selector lists | `h1, h2`, `main > h1, main > h2` | |
| Logical pseudo-classes | `:not(...)`, `:is(...)`, `:where(...)` | Arguments must be local compound selectors |
| Child ordinals | `:first-child`, `:nth-child(2n+1)` | |
| Type ordinals | `:first-of-type`, `:nth-of-type(2)` | |
| Filtered ordinals | `:nth-child(2 of .card, [data-card])` | `of S` must be a local compound selector |
| Root and scope | `:root`, `:scope > a` | `:scope` is the nested query's parent match |

scah evaluates selectors as tags open. Rather than silently narrowing results,
it rejects selectors it cannot decide at that point:

- **Future-dependent selectors** are rejected: `:has()`, `:empty`,
  `:last-child`, and `:nth-last-child()`.
- **Inside `:is()` and `:where()`**, recoverable invalid alternatives are
  dropped as CSS specifies, so `div:is(.card, .bad])` matches `div.card`. A
  valid alternative that scah cannot evaluate rejects the whole selector. That
  covers combinators, structural or unrecognized pseudo-classes, escaped
  pseudo-class names, out-of-range `An+B` numbers, escaped or non-ASCII
  identifiers (including attribute names), escaped attribute values such as
  `[title="a\"b"]`, and namespaced attributes such as `[ns|attr]`.
- **`:not()` and `of S` lists are strict**: any invalid or unsupported
  alternative rejects the selector.
- **Unmodeled syntax fails closed**, even inside nested `:not()`, `:is()`, or
  `:where()`: CSS escapes (`\`), comments (`/* */`), the nesting selector `&`,
  NUL characters, and namespace `|` outside attribute brackets. For example,
  `div:is(.card, .bad]/**/)` fails with "CSS comments are not supported".
- **`:scope`** is supported only as a standalone leading anchor in nested
  queries, as in `:scope > a`. Terminal `:scope` and compound anchors such as
  `:scope.card` are rejected.
- **Nesting depth**: selector lists inside functional pseudo-classes may nest
  up to `MAX_SELECTOR_NESTING_DEPTH` (32) levels.

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

## Text extraction

| Field | Meaning |
| ----- | ------- |
| `inner_html` / `innerHtml` | Raw markup between the element's tags |
| `raw_text` / `rawText` | Source-preserving descendant text. Whitespace and entity spellings are kept and markup is omitted. Includes `script`, `style`, `template`, and `hidden` subtrees. |
| `text` | Normalized, human-readable descendant text (details below). This is **not** browser `innerText`; scah does not evaluate CSS. |

Normalized `text`:

- Omits `script`, `style`, `template`, and elements with a `hidden` attribute,
  including any structural separators they would contribute.
- Inserts line breaks for blocks, `<br>`, and `<hr>`, newlines between table
  rows, and tabs between table cells. Other elements concatenate inline.
- Preserves preformatted whitespace in `pre` and `textarea`, including literal
  or decoded nonbreaking spaces, after HTML's rule that drops a newline
  immediately following the start tag.
- Decodes HTML character references: named, numeric, legacy semicolon-less
  forms in data state, C1 remapping, and U+FFFD for invalid scalars.

"Block" is a fixed set of structural elements: `address`, `article`, `aside`,
`blockquote`, `body`, `caption`, `colgroup`, `dd`, `details`, `dialog`, `div`,
`dl`, `dt`, `fieldset`, `figcaption`, `figure`, `footer`, `form`, `h1`–`h6`,
`header`, `hgroup`, `legend`, `li`, `main`, `menu`, `nav`, `ol`, `p`, `pre`,
`search`, `section`, `summary`, `table`, `tbody`, `tfoot`, `thead`, and `ul`.

`None` / `null` means the query did not request that representation; an empty
string means it was requested and the element had no text.

`Save::all()` captures all three representations. Prefer `Save::only_text()` or
`Save::only_raw_text()` when you need just one.

> **Breaking change:** `text` replaced `text_content` in 0.1.0, and its output differs. If you
> store or compare extracted text, see the [migration notes](https://github.com/zacharyvmm/scah/blob/main/CHANGELOG.md).

## Benchmarks

### Rust

<!-- benchmarks:rust -->
| Library | WHATWG spec | Nested (all) | Nested (first) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: | ---: |
| **scah** | **20.8 ms** | **4.32 ms** | **1.77 µs** | **1.56 ms** | **619 ns** |
| lol_html | 26.9 ms (1.29×) | 8.39 ms (1.94×) | 2.82 µs (1.59×) | 2.49 ms (1.60×) | 663 ns (1.07×) |
| tl | 33.6 ms (1.61×) | 10.8 ms (2.49×) | 2.46 ms (1,392×) | 3.35 ms (2.15×) | 1.72 ms (2,784×) |
| lexbor | 60.6 ms (2.91×) | 10.3 ms (2.40×) | 5.75 ms (3,253×) | 4.74 ms (3.04×) | 3.43 ms (5,529×) |
| scraper | 137 ms (6.60×) | 23.0 ms (5.34×) | 12.0 ms (6,813×) | 13.6 ms (8.73×) | 12.0 ms (19,333×) |
| lxml | 328 ms (15.7×) | 60.6 ms (14.0×) | 43.2 ms (24,456×) | 23.4 ms (15.0×) | 17.5 ms (28,328×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-03; raw data and run details in [`benches/results/rust`](https://github.com/zacharyvmm/scah/tree/main/benches/results/rust).
<!-- /benchmarks:rust -->

### Python

<!-- benchmarks:python -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **Scah** | **45.1 ms** | **11.1 ms** | **3.32 ms** | **8.35 µs** |
| Selectolax | 140 ms (3.11×) | 73.4 ms (6.64×) | 7.96 ms (2.39×) | 3.00 ms (359×) |
| lxml | 345 ms (7.66×) | 330 ms (29.8×) | 21.7 ms (6.54×) | 6.54 ms (783×) |
| Parsel | 644 ms (14.3×) | 276 ms (25.0×) | 69.7 ms (21.0×) | 10.4 ms (1,249×) |
| Gazpacho | 1.57 s (34.9×) | — | 120 ms (36.0×) | 64.7 ms (7,739×) |
| BS4 (lxml) | 2.43 s (53.9×) | 817 ms (73.9×) | 101 ms (30.4×) | 80.6 ms (9,650×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-03; raw data and run details in [`benches/results/python`](https://github.com/zacharyvmm/scah/tree/main/benches/results/python).
<!-- /benchmarks:python -->

### JavaScript (Bun)

<!-- benchmarks:node -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **51.8 ms** | **26.3 ms** | **5.46 ms** | **58.3 µs** |
| linkedom | 431 ms (8.33×) | 67.4 ms (2.57×) | 50.3 ms (9.20×) | 85.8 ms (1,472×) |
| cheerio | 517 ms (9.99×) | 90.9 ms (3.46×) | 32.5 ms (5.94×) | 24.1 ms (413×) |
| node-html-parser | 667 ms (12.9×) | 115 ms (4.37×) | 159 ms (29.2×) | 31.2 ms (535×) |
| jsdom | — | 365 ms (13.9×) | 179 ms (32.8×) | 297 ms (5,103×) |
| happy-dom | — | — | 124 ms (22.7×) | 125 ms (2,148×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-03; raw data and run details in [`benches/results/node`](https://github.com/zacharyvmm/scah/tree/main/benches/results/node).
<!-- /benchmarks:node -->

See [`benches/`](https://github.com/zacharyvmm/scah/blob/main/benches/README.md) for what each scenario measures and how to
reproduce these numbers.

## Contributing

See [CONTRIBUTING.md](https://github.com/zacharyvmm/scah/blob/main/CONTRIBUTING.md). Changes are recorded in
[CHANGELOG.md](https://github.com/zacharyvmm/scah/blob/main/CHANGELOG.md).

## License

scah is available under the [MIT License](https://github.com/zacharyvmm/scah/blob/main/LICENSE). The HTML entity table is
derived from the WHATWG HTML Standard; see
[`crates/scah/THIRD_PARTY_LICENSES`](https://github.com/zacharyvmm/scah/blob/main/crates/scah/THIRD_PARTY_LICENSES/WHATWG-HTML.txt).
