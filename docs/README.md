# scah documentation

Start with the [project README](../README.md) for installation and a quick
start in Rust, Python, and JavaScript.

## Guides

- **[Selector support](selectors.md)**: every supported selector, what is
  rejected and why, and how nested queries scope their selectors.
- **[Text extraction](text-extraction.md)**: what `inner_html`, `raw_text`,
  and `text` contain, and how to save only what you need.
- **[Architecture](architecture.md)**: how a parse works, where the speed
  comes from, and which crate does what.

## Reference

- **Rust API**: [docs.rs/scah](https://docs.rs/scah)
- **Python**: [package README](../crates/bindings/scah-python/README.md) and
  the bundled type stubs ([`scah.pyi`](../crates/bindings/scah-python/scah.pyi))
- **JavaScript and TypeScript**: [package README](../crates/bindings/scah-node/README.md)
  and the bundled type definitions ([`index.d.ts`](../crates/bindings/scah-node/index.d.ts))
- **Examples**: [`crates/scah/examples`](../crates/scah/examples)

## Project

- [Changelog](../CHANGELOG.md), including migration notes for breaking releases
- [Benchmarks](../benches/README.md): what each scenario measures and how to
  reproduce the results
- [Contributing](../CONTRIBUTING.md)
- [Security policy](../SECURITY.md)
