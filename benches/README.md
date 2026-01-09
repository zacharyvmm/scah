# Performance Benchmark Report
![Benchmark Image](./images/criterion_benches.png)

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.049088** | 0.000399 |
| lol_html | **0.062062** | 0.000441 |
| tl | **0.063642** | 0.000518 |
| lexbor | **0.149248** | 0.006001 |
| scraper | **0.353872** | 0.002315 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.457360** | 0.004153 |
| tl | **0.562708** | 0.003755 |
| lol_html | **0.606717** | 0.003384 |
| lexbor | **1.191571** | 0.007378 |
| scraper | **3.504726** | 0.012261 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **4.502841** | 0.034677 |
| tl | **6.013767** | 0.025309 |
| lol_html | **6.139372** | 0.020678 |
| lexbor | **13.131345** | 0.044728 |
| scraper | **36.474671** | 0.087426 |

---

## Simple First Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000776** | 0.000008 |
| lol_html | **0.001344** | 0.000013 |
| tl | **0.027132** | 0.000080 |
| lexbor | **0.109545** | 0.000762 |
| scraper | **0.314743** | 0.001010 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000754** | 0.000005 |
| lol_html | **0.001470** | 0.000031 |
| tl | **0.280862** | 0.001290 |
| lexbor | **0.921844** | 0.007854 |
| scraper | **3.357639** | 0.043364 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000831** | 0.000033 |
| lol_html | **0.001421** | 0.000009 |
| tl | **3.039043** | 0.059616 |
| lexbor | **10.141690** | 0.220713 |
| scraper | **32.612211** | 0.143793 |

---

