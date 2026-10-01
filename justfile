set shell := ["bash", "-c"]

default:
    @just --list

build: build-rust build-node build-python
build-rust:
    cargo build --release
build-node:
    cd crates/bindings/scah-node && bun install && bun run build
build-python:
    cd crates/bindings/scah-python && cargo run --bin stub_gen && uvx maturin build --release

dev: dev-rust dev-node dev-python
dev-rust:
    cargo build
dev-node:
    cd crates/bindings/scah-node && bun install && bun run build:debug
dev-python:
    cd crates/bindings/scah-python && cargo run --bin stub_gen && uvx maturin build

test: test-rust test-node test-python
test-rust:
    cargo test --all-targets --all-features
test-node:
    cd crates/bindings/scah-node && bun test
test-python:
    cd crates/bindings/scah-python && uv run --with pytest --reinstall-package scah pytest tests/

format:
    cargo fmt --all
    cd crates/bindings/scah-node && bun run format

lint:
    cargo clippy --all-targets --all-features -- -D warnings
    cd crates/bindings/scah-node && bun run lint

# Run every comparison benchmark into benches/results, then refresh README tables
bench: bench-rust bench-node bench-python bench-readme
bench-readme:
    python3 benches/report/report.py readme README.md crates/bindings/scah-python/README.md crates/bindings/scah-node/README.md

# Requires cargo-criterion: cargo install cargo-criterion
bench-rust:
    cargo criterion -p scah-benches --message-format=json --bench speed_bench_simple_all --bench speed_bench_simple_first --bench speed_bench_nested_queries --bench speed_bench_spec_all_links > criterion.json
    python3 benches/report/report.py import-criterion criterion.json
bench-rust-simple-all:
    cargo bench -p scah-benches --bench speed_bench_simple_all
bench-rust-first:
    cargo bench -p scah-benches --bench speed_bench_simple_first
bench-rust-whatwg:
    cargo bench -p scah-benches --bench speed_bench_spec_all_links
bench-rust-nested:
    cargo bench -p scah-benches --bench speed_bench_nested_queries

bench-node: (bench-node-scenario "simple-all" "10000") (bench-node-scenario "simple-first" "10000") (bench-node-scenario "nested-all" "10000") (bench-node-scenario "whatwg-all-links")
bench-node-scenario scenario size="":
    cd crates/bindings/scah-node && bun benchmark/bench.ts --scenario {{scenario}} --json benchmark/results/{{scenario}}.json
    python3 benches/report/report.py import-pytest crates/bindings/scah-node/benchmark/results/{{scenario}}.json --suite node --scenario {{scenario}} --runtime "bun $(bun --version)" {{ if size != "" { "--size " + size } else { "" } }}

bench-python: (bench-python-scenario "test_synthetic.py" "simple-all" "10000") (bench-python-scenario "test_synthetic_first.py" "simple-first" "10000") (bench-python-scenario "test_structural.py" "nested-all" "10000") (bench-python-scenario "test_spec.py" "whatwg-all-links")
bench-python-scenario test scenario size="":
    cd crates/bindings/scah-python && uv run --all-extras pytest benches/{{test}} --benchmark-columns=min,mean,max --benchmark-sort=mean --benchmark-warmup-iterations 5 --benchmark-json benches/{{scenario}}.json
    python3 benches/report/report.py import-pytest crates/bindings/scah-python/benches/{{scenario}}.json --suite python --scenario {{scenario}} {{ if size != "" { "--size " + size } else { "" } }}
    rm crates/bindings/scah-python/benches/{{scenario}}.json

# Performance gates: compare this checkout against a base revision (x86-64 only)
gate-sibling base="origin/main":
    ./benches/gates/check-sibling-performance.sh {{base}}
gate-text base="origin/main":
    ./benches/gates/check-text-performance.sh {{base}}
download-html-spec-bench:
    mkdir -p benches/bench_data
    curl -L "https://html.spec.whatwg.org/" -o benches/bench_data/html.spec.whatwg.org.html

bump new_version:
    just bump-rust "{{new_version}}"
    just bump-node "{{new_version}}"
    just bump-python "{{new_version}}"
    cargo check
bump-rust new_version:
    sed -E -i.bak 's/^(version = |scah(-[a-z-]+)? = \{ version = )"[^"]*"/\1"{{new_version}}"/' Cargo.toml
    sed -i.bak 's/^scah = "[^"]*"/scah = "{{new_version}}"/' README.md
    rm -f Cargo.toml.bak README.md.bak
bump-node new_version:
    sed -i.bak 's/^  "version": "[^"]*",/  "version": "{{new_version}}",/' crates/bindings/scah-node/package.json
    sed -E -i.bak '/^  "optionalDependencies": \{/,/^  \}/ s/^    ("@zacharymm\/scah-[^"]+": )"[^"]+"(,?)$/    \1"{{new_version}}"\2/' crates/bindings/scah-node/package.json
    rm -f crates/bindings/scah-node/package.json.bak
bump-python new_version:
    sed -i.bak 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/bindings/scah-python/pyproject.toml
    rm -f crates/bindings/scah-python/pyproject.toml.bak
trigger-release new_version:
    git tag -a v{{new_version}} -m "Version {{new_version}} release"
    git push origin v{{new_version}}

code-cov:
    # cargo llvm-cov --html
    cargo llvm-cov
