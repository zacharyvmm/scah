# Benchmarks

## Layout

| Path | Contents |
| ---- | -------- |
| `simple/`, `nested/`, `spec/` | Cross-library comparisons (scah vs `lol_html`, `tl`, lexbor, `scraper`, lxml) |
| `macro/` | Runtime query builder vs the `query!` macro (currently disabled in `Cargo.toml`) |
| `text_extraction/` | Text-mode throughput and memory, plus the text performance gate |
| `cursor_dominance/`, `sibling/`, `structural/`, `ordinary_gate/` | Parser engine workloads; `ordinary_gate` backs the sibling performance gate |
| `element_tape/`, `escape_scanner/`, `tag_classification/` | Microbenchmarks for internal design experiments |
| `gates/` | Scripts that compare a pull request against its base revision in CI |
| `results/` | Published comparison results, one JSON file per suite and scenario |
| `report/` | Converts raw benchmark output into `results/` and renders the README tables |

The Python and Node comparisons live with their bindings, in
`crates/bindings/scah-python/benches/` and `crates/bindings/scah-node/benchmark/`.

## Running

The WHATWG scenario parses a local copy of the HTML specification:

```bash
just download-html-spec-bench
```

Run a single Rust comparison while iterating:

```bash
just bench-rust-simple-all   # or bench-rust-first, bench-rust-nested, bench-rust-whatwg
```

Record published results and refresh the README tables:

```bash
just bench-rust     # needs cargo-criterion: cargo install cargo-criterion
just bench-python
just bench-node
just bench-readme
```

Commit the updated `results/` files together with the README changes. Each
file records the date, commit, OS, CPU, and runtime of its run; record results
on an otherwise idle machine.

Memory benchmarks use gungraun (Valgrind) and run only on Linux:

```bash
cargo bench -p scah-benches --features linux-memory-benches --bench memory_bench_simple_all
```

## Result format

`results/<suite>/<scenario>.json`, where `suite` is `rust`, `python`, or
`node`:

```json
{
  "schema_version": 1,
  "suite": "rust",
  "scenario": "simple-all",
  "title": "Every <a> in a flat list of <div><a> pairs",
  "unit": "ms",
  "environment": {
    "date": "2026-06-06",
    "commit": "…",
    "dirty": false,
    "os": "Linux 7.0.0",
    "arch": "x86_64",
    "cpu": "…",
    "runtime": "rustc 1.98.1"
  },
  "results": [
    { "library": "scah", "size": 10000, "mean": 1.92, "stdev": 0.01 }
  ]
}
```

`mean` and `stdev` are milliseconds per parse-and-query. `size` is the number of
generated elements for synthetic inputs and `null` for real documents. Files
imported from the earlier PNG reports carry an `environment.note` and no
machine details.
