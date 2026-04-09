# Nodejs/Bun bindings for scan HTML

```bash
npm install scah@npm:@zacharymm/scah
```

## Benchmark

### Simple Html BenchMark (select all `a` tags):
![Simple Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-node/benchmark/images/synthetic.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| scah | **13.871424** | 1.577820 | 1x |
| linkedom | **83.437088** | 13.218471 | 6.02x |
| cheerio | **93.475415** | 9.873812 | 6.74x |
| happy-dom | **269.044447** | 24.372608 | 19.4x |
| jsdom | **343.614964** | 38.366862 | 24.77x |
| node-html-parser | **470.989438** | 24.691218 | 33.95x |


### Simple First Html BenchMark (find first `a` tag):
![Simple First Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-node/benchmark/images/synthetic_first.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| scah | **0.247321** | 0.017258 | 1x |
| node-html-parser | **64.092075** | 9.236343 | 259.14x |
| cheerio | **68.177594** | 5.570901 | 275.66x |
| linkedom | **146.949830** | 20.281012 | 594.17x |
| happy-dom | **251.732231** | 31.785607 | 1017.83x |
| jsdom | **291.487428** | 33.279401 | 1178.58x |

### WHATWG Html BenchMark (find all `a` tags):
![WHATWG Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-node/benchmark/images/whatwg.png)

| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| scah | **125.980227** | 12.241186 | 1x |
| linkedom | **1,255.033456** | 512.454055 | 9.96x |
| cheerio | **1,336.089538** | 53.096898 | 10.61x |
| node-html-parser | **1,953.938280** | 43.960253 | 15.51x |

### Nested Html BenchMark (find all products)
![Nested Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-node/benchmark/images/nested.png)


| Library | Mean (ms) | stdev | multiplier |
| :--- | :--- | :--- | :--- |
| scah | **71.643558** | 6.181472 | 1x |
| linkedom | **153.298287** | 16.085323 | 2.14x |
| cheerio | **265.930798** | 12.760859 | 3.71x |
| node-html-parser | **389.224348** | 95.301039 | 5.43x |
| jsdom | **878.792820** | 97.970828 | 12.27x |
