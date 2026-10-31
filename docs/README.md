# scah documentation

Start with the [project README](../README.md) for installation and a quick
start in each language.

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
  the bundled type stubs ([`_scah.pyi`](../crates/bindings/scah-python/python/scah/_scah.pyi))
- **JavaScript and TypeScript**: [package README](../crates/bindings/scah-node/README.md)
  and the bundled type definitions ([`index.d.ts`](../crates/bindings/scah-node/index.d.ts))
- **Java**: [README](../crates/bindings/scah-java/README.md)
- **.NET**: [README](../crates/bindings/scah-csharp/README.md)
- **C and C++**: [README](../crates/bindings/scah-c/README.md) and the
  documented header ([`scah.h`](../crates/bindings/scah-c/include/scah.h))
- **Arrow export and polars**: the [project README](../README.md#arrow-export)
  and the [Python README](../crates/bindings/scah-python/README.md#polars)
- **Examples**: [`crates/scah/examples`](../crates/scah/examples)

## Project

- [Changelog](../CHANGELOG.md), including migration notes for breaking releases
- [Benchmarks](../benches/README.md): what each scenario measures and how to
  reproduce the results
- [Contributing](../CONTRIBUTING.md)
- [Security policy](../SECURITY.md)
