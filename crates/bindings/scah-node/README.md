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
| **scah** | **146 ms** | **72.7 ms** | **15.2 ms** | **247 µs** |
| cheerio | 1.44 s (9.85×) | 253 ms (3.48×) | 87.4 ms (5.73×) | 68.2 ms (276×) |
| node-html-parser | 1.89 s (12.9×) | 393 ms (5.41×) | 462 ms (30.3×) | 64.1 ms (259×) |
| linkedom | 6.57 s (44.9×) | 218 ms (3.00×) | 96.3 ms (6.32×) | 147 ms (595×) |
| jsdom | — | 1.01 s (13.9×) | 404 ms (26.5×) | 291 ms (1,180×) |
| happy-dom | — | — | 320 ms (21.0×) | 252 ms (1,019×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-04-08 to 2026-07-15; raw data and run details in [`benches/results/node`](https://github.com/zacharyvmm/scah/tree/main/benches/results/node).
<!-- /benchmarks:node -->

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
