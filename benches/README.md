# Performance Benchmark Report
![Benchmark Image](./images/criterion_benches.png)

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.042184** | 0.000206 |
| tl | **0.056916** | 0.000412 |
| lol_html | **0.059896** | 0.000431 |
| lexbor | **0.140394** | 0.002813 |
| scraper | **0.345760** | 0.001221 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.404985** | 0.005679 |
| tl | **0.557617** | 0.008518 |
| lol_html | **0.614523** | 0.000903 |
| lexbor | **1.208808** | 0.004541 |
| scraper | **3.528352** | 0.011533 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **3.857509** | 0.024504 |
| tl | **5.823485** | 0.006550 |
| lol_html | **6.140056** | 0.009504 |
| lexbor | **13.231017** | 0.048440 |
| scraper | **36.212745** | 0.063966 |

---

## Simple First Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000780** | 0.000002 |
| lol_html | **0.001372** | 0.000005 |
| tl | **0.027106** | 0.000100 |
| lexbor | **0.107064** | 0.000736 |
| scraper | **0.311573** | 0.001131 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000769** | 0.000005 |
| lol_html | **0.001371** | 0.000007 |
| tl | **0.287096** | 0.000609 |
| lexbor | **0.887518** | 0.003491 |
| scraper | **3.133357** | 0.008357 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000786** | 0.000003 |
| lol_html | **0.001336** | 0.000005 |
| tl | **2.816879** | 0.006791 |
| lexbor | **9.609559** | 0.081585 |
| scraper | **31.937846** | 0.123806 |

---

