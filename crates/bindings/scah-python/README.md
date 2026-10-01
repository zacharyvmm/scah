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
| **Scah** | **116 ms** | **25.2 ms** | **8.61 ms** | **47.0 µs** |
| Selectolax | 340 ms (2.94×) | 162 ms (6.43×) | 21.2 ms (2.46×) | 9.23 ms (196×) |
| lxml | 769 ms (6.65×) | 970 ms (38.4×) | 53.0 ms (6.16×) | 15.9 ms (337×) |
| Parsel | 1.80 s (15.5×) | 767 ms (30.4×) | 200 ms (23.2×) | 25.8 ms (550×) |
| Gazpacho | 3.81 s (33.0×) | — | 556 ms (64.6×) | 156 ms (3,325×) |
| BS4 (lxml) | 5.45 s (47.1×) | 2.15 s (85.3×) | 248 ms (28.8×) | 246 ms (5,235×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-04-08 to 2026-07-15; raw data and run details in [`benches/results/python`](https://github.com/zacharyvmm/scah/tree/main/benches/results/python).
<!-- /benchmarks:python -->

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
