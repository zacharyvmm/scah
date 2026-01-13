# Performance Benchmark Report
![Benchmark Image](./images/criterion_benches.png)

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.036054** | 0.000150 |
| tl | **0.058056** | 0.000308 |
| lol_html | **0.060664** | 0.000332 |
| lexbor | **0.138901** | 0.000741 |
| scraper | **0.349017** | 0.004090 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.343413** | 0.004473 |
| tl | **0.554674** | 0.003055 |
| lol_html | **0.608035** | 0.002202 |
| lexbor | **1.170863** | 0.008220 |
| scraper | **3.541929** | 0.010431 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **3.489262** | 0.018170 |
| tl | **5.887970** | 0.009246 |
| lol_html | **5.903961** | 0.034279 |
| lexbor | **13.395031** | 0.102061 |
| scraper | **36.677289** | 0.069227 |

---

## Simple First Selection Comparison

### Input Size: 100 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000722** | 0.000004 |
| lol_html | **0.001317** | 0.000007 |
| tl | **0.031020** | 0.000901 |
| lexbor | **0.108961** | 0.000386 |
| scraper | **0.324607** | 0.009518 |

### Input Size: 1000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000743** | 0.000011 |
| lol_html | **0.001341** | 0.000015 |
| tl | **0.279087** | 0.001614 |
| lexbor | **0.872930** | 0.006882 |
| scraper | **3.168580** | 0.007766 |

### Input Size: 10000 Elements

| Library | Min (ms) | StdDev (ms) |
| :--- | :--- | :--- |
| onego | **0.000739** | 0.000003 |
| lol_html | **0.001384** | 0.000006 |
| tl | **2.863881** | 0.006064 |
| lexbor | **9.618386** | 0.015514 |
| scraper | **32.405031** | 0.061824 |

---

