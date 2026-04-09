# Python Bindings for scah

## Benchmark
### Real Html BenchMark ([html.spec.whatwg.org](https://html.spec.whatwg.org/)) (select all `a` tags):
![WhatWg Html Spec BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/whatwg.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| Scah | **100.453112** | 5.285644 | 1x |
| Selectolax | **666.531062** | 770.046444 | 6.64x |
| lxml | **786.899222** | 6.664833 | 7.83x |
| Parsel | **1,742.625470** | 10.651475 | 17.35x |
| Gazpacho | **4,326.287304** | 112.214583 | 43.07x |
| BS4 (lxml) | **5,836.098065** | 730.163386 | 58.1x |

### Synthetic Html BenchMark (select all `a` tags):
![Synthetic Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/synthetic.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| Scah | **7.391444** | 0.474385 | 1x |
| Selectolax | **18.758350** | 2.495796 | 2.54x |
| lxml | **59.707865** | 0.993740 | 8.08x |
| Parsel | **205.402533** | 0.454010 | 27.79x |
| BS4 (lxml) | **271.418740** | 4.907597 | 36.72x |
| Gazpacho | **325.898600** | 1.559744 | 44.09x |


### First Html BenchMark (find first `a` tag):
![First Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/synthetic_first.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| Scah | **0.042838** | 0.011730 | 1x |
| Selectolax | **8.624247** | 0.053230 | 201.32x |
| lxml | **14.992198** | 0.138058 | 349.98x |
| Parsel | **23.812899** | 1.931582 | 555.89x |
| Gazpacho | **144.887551** | 0.549365 | 3382.26x |
| BS4 (lxml) | **224.398618** | 2.581323 | 5238.37x |

### Nested Html BenchMark (select all `Products`):
![Nested Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/nested.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| Scah | **24.552884** | 0.192680 | 1x |
| Selectolax | **140.404934** | 0.426443 | 5.72x |
| Parsel | **807.709630** | 183.818712 | 32.9x |
| lxml | **899.849129** | 3.819371 | 36.65x |
| BS4 (lxml) | **2,397.357205** | 328.807222 | 97.64x |