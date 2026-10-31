# scah for Python

**Fast HTML extraction with CSS selectors. One pass, no DOM.**

[![PyPI](https://img.shields.io/pypi/v/scah)](https://pypi.org/project/scah/)
[![Python versions](https://img.shields.io/pypi/pyversions/scah)](https://pypi.org/project/scah/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/zacharyvmm/scah/blob/main/LICENSE)

scah pulls the elements you select out of an HTML document in a single
forward pass. You write CSS selectors; scah streams the document through a
Rust parser once and keeps only the matches. There is no tree to build, so it
is fast on large pages: finding every link in the 15 MB WHATWG HTML
specification is about 4× faster than selectolax and 50× faster than
BeautifulSoup.

```bash
pip install scah
```

Wheels are available for CPython 3.10+ on Linux (x86-64, ARM64), macOS (Intel,
Apple silicon), and Windows (x86-64). The package ships type stubs.

## Quick start

```python
from scah import Query, Save, parse

html = """
<ul>
  <li><a href="/one">One</a></li>
  <li><a href="/two">Two</a></li>
</ul>
"""

store = parse(html, [Query.all("a[href]", Save.only_text()).build()])
for link in store.get("a[href]"):
    print(link.text, link.get_attribute("href"))
# One /one
# Two /two
```

## Structured extraction

Child queries added with `.then()` run inside each match of their parent, so
every record keeps its own fields:

```python
from scah import Query, Save, parse

html = """
<article class="product"><h2>Desk lamp</h2><span class="price">$24</span></article>
<article class="product"><h2>Office chair</h2><span class="price">$189</span></article>
"""

query = (
    Query.all("article.product", Save.none())
    .then(lambda product: [
        product.first("> h2", Save.only_text()),
        product.first(".price", Save.only_text()),
    ])
    .build()
)

store = parse(html, [query])
for product in store.get("article.product"):
    name = product.get("> h2")[0].text
    price = product.get(".price")[0].text
    print(f"{name}: {price}")
# Desk lamp: $24
# Office chair: $189
```

## API

### Building queries

| Call | Returns |
| ---- | ------- |
| `Query.all(selector, save)` | A builder for a query that keeps every match |
| `Query.first(selector, save)` | A builder for a query that keeps the first match |
| `builder.then(fn)` | Adds the child queries `fn(factory)` returns; `factory.all()` and `factory.first()` build them |
| `builder.all(selector, save)`, `builder.first(selector, save)` | Adds one child query under the last selector |
| `builder.build()` | The finished query; raises `ValueError` for a selector scah can't evaluate |
| `parse(html, queries)` | A `Store` with the results of every query, from one pass |

Build a query once and reuse it with `parse` for as many documents as you
like.

### What to save

| `Save` | Content kept for each match |
| ------ | --------------------------- |
| `Save.all()` | Inner HTML, raw text, and normalized text |
| `Save.only_text()` | Normalized, readable text |
| `Save.only_raw_text()` | Source text with markup removed |
| `Save.only_inner_html()` | The markup inside the element |
| `Save.none()` | No content |
| `Save.name_only()` | No content and no attributes |
| `Save(inner_html=False, text=False, attributes=True, *, raw_text=False)` | Any combination |

Every option except `name_only` and `attributes=False` keeps the element's id,
class, and attributes. Save only what you read: each extra field costs time.

### Reading results

| `Store` | |
| ------- | - |
| `store.get(selector)` | The matches of the first query with that selector, or `None` if it matched nothing or no query has it |
| `store.query(index)` | The matches of the query at `index` in the list passed to `parse`, or `None` as for `get` |
| `len(store)` | The number of saved elements |

| `Element` | |
| --------- | - |
| `name`, `id`, `class_name` | The tag name, `id`, and `class` attribute |
| `attributes`, `get_attribute(name)` | The other attributes, as a `dict` or one at a time |
| `inner_html`, `raw_text`, `text` | Saved content, or `None` if the query didn't save it |
| `get(selector)` | The element's matches for a child query; raises `ValueError` if there are none |

## Selectors and text

scah supports the CSS selectors that can be decided in one forward pass:
type, id, class, attribute, all four combinators, selector lists, `:not()`,
`:is()`, `:where()`, `:nth-child()`, `:nth-of-type()`, and more. Selectors
that need to look ahead, such as `:has()` and `:last-child`, raise
`ValueError` instead of returning partial results.

- [Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md)
- [Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md):
  what `inner_html`, `raw_text`, and `text` contain

## Benchmarks

Mean time to parse a document and run the query, compared with popular
Python HTML libraries.

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

## Links

- [Source and issues](https://github.com/zacharyvmm/scah)
- [Changelog](https://github.com/zacharyvmm/scah/blob/main/CHANGELOG.md)
- scah is also available for [Rust](https://crates.io/crates/scah) and
  [JavaScript](https://www.npmjs.com/package/@zacharymm/scah)

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
