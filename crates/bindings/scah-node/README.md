# scah for JavaScript and TypeScript

**Fast HTML extraction with CSS selectors. One pass, no DOM.**

[![npm](https://img.shields.io/npm/v/%40zacharymm%2Fscah)](https://www.npmjs.com/package/@zacharymm/scah)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/zacharyvmm/scah/blob/main/LICENSE)

scah pulls the elements you select out of an HTML document in a single
forward pass. You write CSS selectors; scah streams the document through a
native Rust parser once and keeps only the matches. There is no DOM to build,
so it is fast on large pages: finding every link in the 15 MB WHATWG HTML
specification is about 8× faster than linkedom and 13× faster than cheerio.

```bash
npm install @zacharymm/scah
```

Native binaries are published for Linux (x86-64, ARM64), macOS (Intel, Apple
silicon), and Windows (x86-64), and work in Node.js and Bun. TypeScript types
are included.

## Quick start

```ts
import { Query, parse } from '@zacharymm/scah'

const html = `
<ul>
  <li><a href="/one">One</a></li>
  <li><a href="/two">Two</a></li>
</ul>`

const store = parse(html, [Query.all('a[href]', { text: true }).build()])
for (const link of store.get('a[href]') ?? []) {
  console.log(link.text, link.getAttribute('href'))
}
// One /one
// Two /two
```

## Structured extraction

Child queries added with `.then()` run inside each match of their parent, so
every record keeps its own fields:

```ts
import { Query, parse } from '@zacharymm/scah'

const html = `
<article class="product"><h2>Desk lamp</h2><span class="price">$24</span></article>
<article class="product"><h2>Office chair</h2><span class="price">$189</span></article>`

const query = Query.all('article.product')
  .then((product) => [product.first('> h2', { text: true }), product.first('.price', { text: true })])
  .build()

const store = parse(html, [query])
for (const product of store.get('article.product') ?? []) {
  const [name] = product.get('> h2')
  const [price] = product.get('.price')
  console.log(`${name.text}: ${price.text}`)
}
// Desk lamp: $24
// Office chair: $189
```

## API

### Building queries

| Call | Returns |
| ---- | ------- |
| `Query.all(selector, save?)` | A builder for a query that keeps every match |
| `Query.first(selector, save?)` | A builder for a query that keeps the first match |
| `builder.then(fn)` | Adds the child queries `fn(factory)` returns; `factory.all()` and `factory.first()` build them |
| `builder.all(selector, save?)`, `builder.first(selector, save?)` | Adds one child query under the last selector |
| `builder.build()` | The finished query; throws for a selector scah can't evaluate |
| `parse(html, queries)` | A `Store` with the results of every query, from one pass |

Build a query once and reuse it with `parse` for as many documents as you
like.

### What to save

The optional second argument chooses what to keep for each match:

```ts
interface Save {
  innerHtml?: boolean // the markup inside the element (default false)
  rawText?: boolean // source text with markup removed (default false)
  text?: boolean // normalized, readable text (default false)
  attributes?: boolean // id, class, and other attributes (default true)
}
```

Save only what you read: each extra field costs time.

### Reading results

| `Store` | |
| ------- | - |
| `store.get(selector)` | The matches of the first query with that selector, or `null` if it matched nothing or no query has it |
| `store.query(index)` | The matches of the query at `index` in the array passed to `parse`, or `null` as for `get` |
| `store.length` | The number of saved elements |

| `Element` | |
| --------- | - |
| `name`, `id`, `className` | The tag name, `id`, and `class` attribute |
| `attributes`, `getAttribute(name)` | The other attributes, as an object or one at a time |
| `innerHtml`, `rawText`, `text` | Saved content, or `null` if the query didn't save it |
| `get(selector)` | The element's matches for a child query; throws if there are none |
| `toJson()` | A plain object with all of the above |

## Selectors and text

scah supports the CSS selectors that can be decided in one forward pass:
type, id, class, attribute, all four combinators, selector lists, `:not()`,
`:is()`, `:where()`, `:nth-child()`, `:nth-of-type()`, and more. Selectors
that need to look ahead, such as `:has()` and `:last-child`, throw instead of
returning partial results.

- [Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md)
- [Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md):
  what `innerHtml`, `rawText`, and `text` contain

## Benchmarks

Mean time to parse a document and run the query in Bun, compared with popular
JavaScript HTML libraries.

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

## Links

- [Source and issues](https://github.com/zacharyvmm/scah)
- [Changelog](https://github.com/zacharyvmm/scah/blob/main/CHANGELOG.md)
- scah is also available for [Rust](https://crates.io/crates/scah) and
  [Python](https://pypi.org/project/scah/)

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
