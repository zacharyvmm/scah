# Contributing to scah

Thanks for helping out. Bug reports, selector edge cases, and benchmark
comparisons are all welcome.

## Setup

You need:

- Rust 1.88 or newer (the CI uses stable)
- [`just`](https://github.com/casey/just) for the task recipes
- For the Python bindings: Python 3.10+ and [`uv`](https://docs.astral.sh/uv/)
- For the Node bindings: [Bun](https://bun.sh)

```bash
just dev     # debug builds of the Rust crate and both bindings
just test    # Rust, Node, and Python test suites
just lint    # clippy with -D warnings, plus oxlint for the Node bindings
just format  # rustfmt and prettier
```

Run `just` to list every recipe.

## Repository layout

| Path | Contents |
| ---- | -------- |
| `crates/scah` | The public crate: parser, query engine, and result store |
| `crates/scah-reader` | Low-level streaming reader primitives |
| `crates/scah-query-ir` | Selector parsing and the compiled query representation |
| `crates/scah-macros` | The `query!` macro |
| `crates/bindings/scah-python` | Python bindings (PyO3 and maturin) |
| `crates/bindings/scah-node` | Node and Bun bindings (napi-rs) |
| `benches` | Benchmarks, published results, and CI performance gates; see [`benches/README.md`](benches/README.md) |

## Pull requests

- Add a test for behavior changes. Parser edge cases belong in
  `crates/scah/tests/html_soup/`.
- Add an entry under `## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) for
  user-visible changes.
- If you change a binding's public API, regenerate the Python stub with
  `cargo run -p scah-python --bin stub_gen`.
- CI runs formatting, clippy, tests on x86-64 and AArch64, docs, and the MSRV
  check. Pull requests that touch more than docs or benchmark results also run
  performance gates that compare the branch against its base.

### Performance gates

The gates are noisy on shared runners. Before opening a performance-sensitive
pull request, run them locally on an x86-64 machine:

```bash
just gate-sibling   # ordinary and sibling selector parsing
just gate-text      # text extraction
```

Both compare the working tree against `origin/main`; pass another revision to
compare against it instead, for example `just gate-text HEAD~1`.

### HTML entity table

`crates/scah/src/html/entities_table.rs` is generated. Don't edit it by hand;
see [`crates/scah/scripts/README.md`](crates/scah/scripts/README.md).

## Releases

Maintainers release by bumping every package version, moving the
`[Unreleased]` changelog entries under the new version, committing, and pushing
a tag:

```bash
just bump 0.0.22
git commit -am "Release 0.0.22"
just trigger-release 0.0.22
```

The tag publishes the crates to crates.io, wheels to PyPI, and the
package to npm.
