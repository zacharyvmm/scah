# Python Bindings for scah

## Benchmark
Real Html BenchMark ([html.spec.whatwg.org](https://html.spec.whatwg.org/)) (select all `a` tags):
![WhatWg Html Spec BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/whatwg.png)

Synthetic Html BenchMark (select all `a` tags):
![Synthetic Html BenchMark](https://raw.githubusercontent.com/zacharyvmm/scah/main/crates/bindings/scah-python/benches/images/synthetic.png)

### Run benchmarks
```bash
uv venv
source .venv/bin/activate
uv run --all-extras poe bench
```
