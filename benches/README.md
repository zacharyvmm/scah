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
| `layout/` | Scripts that separate code-layout effects from code changes (see below) |
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

Record published results and refresh the README tables in one go (needs
`cargo-criterion`, `bun`, and `uv` on `PATH`; install the first with
`cargo install cargo-criterion`):

```bash
just bench
```

This checks the tools up front, rebuilds the Node and Python bindings in release
mode, runs every suite into `results/`, and regenerates the README tables. The
suites can also run one at a time with `just bench-rust`, `just bench-node`,
`just bench-python`, and `just bench-readme`.

Commit the updated `results/` files together with the README changes. Each
file records the date, commit, OS, CPU, and runtime of its run; record results
on an otherwise idle machine.

Memory benchmarks use gungraun (Valgrind) and run only on Linux:

```bash
cargo bench -p scah-benches --features linux-memory-benches --bench memory_bench_simple_all
```

## Code layout

The parse loop is sensitive to where the linker places its code. On Intel
CPUs the share of its µops served from the decoded-µop cache moves by over 20
points between builds that differ only in function order, and its speed moves
with it. A comparison of one build per side, as the CI gates make, can
therefore show a few percent in either direction that no source change
caused. Two scripts help tell the difference:

```bash
# Mean change of this checkout vs a base revision over 4 shuffled layouts
just layout-compare origin/main 4

# The same tree with and without the `#[cold]`/`#[inline(never)]` hints
CANDIDATE_RUSTFLAGS="--cfg scah_no_layout_hints" just layout-compare HEAD 4

# Which functions PGO places in .text.hot / .text.unlikely
just pgo-sections
```

`compare.sh` reports each build's IPC and µop-cache share when `perf` can
read hardware counters (`kernel.perf_event_paranoid` 2 or lower).

The layout hints in `crates/scah/src/html/parser.rs` move rarely run paths
(raw text, mismatched close tags, implied closes, end of input, errors) out
of the parse loop. They are written as
`#[cfg_attr(not(scah_no_layout_hints), cold, inline(never))]`, so building
with `RUSTFLAGS="--cfg scah_no_layout_hints"` removes all of them: use it to
check that they still help, or to train PGO without them and compare its
verdict from `pgo-sections.sh` with where the hints are.

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
