set shell := ["bash", "-c"]

default:
    @just --list

build: build-rust build-node build-python

build-rust:
    cargo build

build-node:
    cd nodejs && bun run build

build-python:
    cd python && uv run maturin build

test: test-rust test-node test-python

test-rust:
    cargo test --all-targets --all-features

test-node:
    cd nodejs && bun test

test-python:
    cd python && uv run pytest

format:
    cargo fmt --all
    cd nodejs && bun run format

lint:
    cargo clippy --all-targets --all-features -- -D warnings
    cd nodejs && bun run lint

bench-rust:
    cargo bench -p scah-benches

bench-rust-criterion:
    cargo criterion --message-format=json >> criterion.json
    python3 ./python/benches/utils/criterion_figure.py ./criterion.json

bench-node:
    cd nodejs && bun run bench

bench-python:
    cd python && uv run --all-extras poe bench

bump new_version:
    just bump-rust "{{new_version}}"
    just bump-node "{{new_version}}"
    just bump-python "{{new_version}}"
    cargo check

trigger-release new_version:
    git tag -a v{{new_version}} -m "Version {{new_version}} release"
    git push origin v{{new_version}}

bump-rust new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' Cargo.toml
    sed -i 's/^scah = "[^"]*"/scah = "{{new_version}}"/' README.md

bump-node new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' nodejs/Cargo.toml
    sed -i 's/^  "version": "[^"]*",/  "version": "{{new_version}}",/' nodejs/package.json
    sed -Ei '/^  "optionalDependencies": \{/,/^  \}/ s/^    ("@zacharymm\/scah-[^"]+": )"[^"]+"(,?)$/    \1"{{new_version}}"\2/' nodejs/package.json

bump-python new_version:
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' python/Cargo.toml
    sed -i 's/^version = "[^"]*"/version = "{{new_version}}"/' python/pyproject.toml
