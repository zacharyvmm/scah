set shell := ["bash", "-c"]

default:
    @just --list

build: build-rust build-node build-python
build-rust:
    cargo build --release
build-node:
    cd crates/bindings/scah-node && bun run build
build-python:
    cd crates/bindings/scah-python && uvx maturin build --release

dev: dev-rust dev-node dev-python
dev-rust:
    cargo build
dev-node:
    cd crates/bindings/scah-node && bun run build:debug
dev-python:
    cd crates/bindings/scah-python && uvx maturin build

test: test-rust test-node test-python
test-rust:
    cargo test --all-targets --all-features
test-node:
    cd crates/bindings/scah-node && bun test
test-python:
    source ./crates/bindings/scah-python/.venv/bin/activate && uv run pytest ./crates/bindings/scah-python/tests/

format:
    cargo fmt --all
    cd crates/bindings/scah-node && bun run format

lint:
    cargo clippy --all-targets --all-features -- -D warnings
    cd crates/bindings/scah-node && bun run lint

bench: bench-rust bench-node bench-python
bench-rust:
    cargo bench -p scah-benches
generate-graph-data:
    cargo criterion -p scah-benches --message-format=json >> criterion.json
generate-graphs:
    source ./crates/bindings/scah-python/.venv/bin/activate && python3 ./crates/bindings/scah-python/benches/utils/criterion_figure.py ./criterion.json
download-html-spec-bench:
    mkdir -p benches/bench_data
    curl -L "https://html.spec.whatwg.org/" -o benches/bench_data/html.spec.whatwg.org.html
bench-node:
    cd crates/bindings/scah-node && bun run bench:image
bench-python:
    cd crates/bindings/scah-python && source .venv/bin/activate && uv run --all-extras poe bench

bump new_version:
    just bump-rust "{{new_version}}"
    just bump-node "{{new_version}}"
    just bump-python "{{new_version}}"
    cargo check
bump-rust new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/scah-reader/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/scah-query-ir/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/scah-macros/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/scah/Cargo.toml
    sed -Ei 's/^(scah-reader = \{ version = )"[^"]*"/\1"{{new_version}}"/' crates/scah-query-ir/Cargo.toml
    sed -Ei 's/^(scah-query-ir = \{ version = )"[^"]*"/\1"{{new_version}}"/' crates/scah-macros/Cargo.toml
    sed -Ei 's/^(scah-reader = \{ version = )"[^"]*"/\1"{{new_version}}"/' crates/scah/Cargo.toml
    sed -Ei 's/^(scah-query-ir = \{ version = )"[^"]*"/\1"{{new_version}}"/' crates/scah/Cargo.toml
    sed -Ei 's/^(scah-macros = \{ version = )"[^"]*"/\1"{{new_version}}"/' crates/scah/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' benches/Cargo.toml
    sed -i 's/^scah = "[^"]*"/scah = "{{new_version}}"/' README.md
bump-node new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/bindings/scah-node/Cargo.toml
    sed -i 's/^  "version": "[^"]*",/  "version": "{{new_version}}",/' crates/bindings/scah-node/package.json
    sed -Ei '/^  "optionalDependencies": \{/,/^  \}/ s/^    ("@zacharymm\/scah-[^"]+": )"[^"]+"(,?)$/    \1"{{new_version}}"\2/' crates/bindings/scah-node/package.json
bump-python new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/bindings/scah-python/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' crates/bindings/scah-python/pyproject.toml
trigger-release new_version:
    git tag -a v{{new_version}} -m "Version {{new_version}} release"
    git push origin v{{new_version}}
