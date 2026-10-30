# scah for JavaScript and TypeScript

> CSS selectors meet streaming HTML parsing. Extract exactly what you select, without building a DOM.

Node.js and Bun bindings for [scah](https://github.com/zacharyvmm/scah), a Rust HTML extraction library. scah
matches CSS selectors in a single forward pass over the document and stores
only the matches.

```bash
npm install @zacharymm/scah
```

## Usage

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

The second argument to `all` and `first` selects what is captured for each
match:

```ts
{
  innerHtml?: boolean
  rawText?: boolean
  text?: boolean
  attributes?: boolean
  textContent?: boolean // deprecated alias for text
}
```

See the [main README](https://github.com/zacharyvmm/scah#selector-support) for supported selectors and
text-extraction semantics.

## Benchmarks

Measured with Bun.

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

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
