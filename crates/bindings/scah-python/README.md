# scah for Python

> CSS selectors meet streaming HTML parsing. Extract exactly what you select, without building a DOM.

Python bindings for [scah](https://github.com/zacharyvmm/scah), a Rust HTML extraction library. scah matches
CSS selectors in a single forward pass over the document and stores only the
matches.

```bash
pip install scah
```

## Usage

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

`Save` controls what is captured for each match: `Save.all()`,
`Save.only_inner_html()`, `Save.only_raw_text()`, `Save.only_text()`, or
`Save.none()` (attributes only). Each element exposes `name`, `id`,
`class_name`, `attributes`, `inner_html`, `raw_text`, `text`, and
`get(selector)` for nested query results.

See the [main README](https://github.com/zacharyvmm/scah#selector-support) for supported selectors and
text-extraction semantics.

## Benchmarks

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

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
