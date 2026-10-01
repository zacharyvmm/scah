# Python Bindings for scah

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
