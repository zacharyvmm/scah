# Changelog

All notable changes to scah are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions apply to the
Rust crates and the Python and npm packages together.

## [Unreleased]

### Breaking

- The cursor engine and `QueryMultiplexer` are removed. `XHtmlParser::new` and
  `XHtmlParser::with_capacity` take the query slice directly, and debug trace
  events describe compiled steps instead of cursors. ([#95])
- Results are stored in flat tables. `Element` no longer has `next_sibling` or
  `first_child_query`, and `Store::push`, `Store::queries`, `QueryNode`, and
  `QueryId` are removed. An element saved by a nested query under several
  parent matches is stored once and listed under each of them. ([#97])
- `Position`, the `QuerySpec` navigation methods, `Transition::next`, and
  `exit_at_section_end` are removed from the query IR; `QuerySpec` only exposes
  query data. ([#98])
- `Element` is replaced by `ElementRef`, a handle to a row of the store.
  Fields become methods (`name()`, `class()`, `id()`, `inner_html()`), and
  accessors no longer take the store: `element.text()`,
  `element.attribute("href")`, `element.get(selector)`. `Store::get` and
  `Store::query` return `Elements`, an iterator of handles, and the public
  `Store::elements` and `Store::attributes` fields give way to
  `Store::element(id)`, `Store::elements()`, and `Store::len()`.
  `element.attributes()` returns `Attributes`, an iterator over the same
  attributes as before (all but the `class` and `id` that have their own
  methods), instead of a slice. ([#100])
- HTML of 4 GiB or more returns `ParseError::InputTooLarge`. ([#100])
- `XHtmlParser::next` panics if stepped over a different source than its
  first call. ([#100])
- The `scah-reader` crate is replaced by `scah-html`, which also holds the tag
  lexer and the open-element tree. `Attribute`, `TagId`, `AttributeMask`, and
  `Reader` move there from `scah-query-ir`, and `scah-macros` no longer exports
  `simd_nibble_tables!`. `scah` still re-exports `Reader`, `Attribute`, and
  `XHtmlElement`. ([#112])
- `Reader` only reads UTF-8, so it can no longer return a string that is not
  valid UTF-8: `Reader::from_bytes` returns an error for other bytes,
  `Reader::source_str` returns `&str`, and `Reader::slice` panics on a range
  that splits a character. `XHtmlElement::parse_attributes` is `unsafe`,
  because the attributes it lists point into a tape it doesn't borrow.
  ([#112])
- The result store moves to a new `scah-store` crate, which knows nothing of
  queries. The `scah` API is unchanged except for the debug trace:
  `Store::trace` is removed, and `scah::debug::parse_traced` returns the store
  together with its `TraceStore`. ([#113])

### Added

- `Store::query(index)` and `Element::nested(&store, index)` select results by
  query position, and `Store::results`, `query_results`, `child_results`, and
  `nested_results` return element ids. Python and Node gain `Store.query`.
  ([#97])
- `parse_compiled` and `parse_compiled_without_text_capture` run a `Program`
  compiled once with `Program::compile`, for parsing many documents with the
  same queries. ([#98])
- A C ABI in `crates/bindings/scah-c`, with a generated `scah.h` header and
  C and C++ smoke tests. It shares its owned query and store handles with the
  Python and Node bindings. ([#48], [#49], [#53])
- `scah_store_query` looks up C results by query position, like
  `Store.query` in Python and Node. `crates/bindings/native/build.sh` builds
  `libscah_c` for Linux (glibc 2.17+ and musl), macOS, and Windows on x64 and
  arm64. ([#122])
- A .NET binding, the `Scah` NuGet package, in `crates/bindings/scah-csharp`.
  It calls the C ABI through `LibraryImport`, targets .NET 8 and newer, is
  Native AOT-compatible, and ships the native library for every platform
  `libscah_c` builds for. ([#123])
- A Java binding, `io.github.zacharyvmm:scah`, in `crates/bindings/scah-java`.
  It calls the C ABI through the Foreign Function and Memory API on Java 22
  and newer, and extracts the native library for its platform from the jar.
  ([#124]) On Java 11 to 21 it uses JNI, through `crates/bindings/scah-jni`.
  ([#125])
- Arrow export: `Store.to_arrow()` in Python and `scah_store_export_arrow` in C
  hand matches over as an Arrow table whose string columns point into the
  parsed HTML and text buffers, ready for polars, pyarrow, DuckDB, and other
  Arrow consumers. In Rust, `Store::field_range` and `Store::buffer` read any
  string field as a byte range of the buffer it lives in.

### Changed

- Queries run as one compiled bitset automaton instead of per-query cursors:
  every query, section, and selector-list alternative is compiled into one
  `Program`, and each open element carries a few bitsets. Deeply nested
  descendant chains and nested `.then()` scopes parse up to 15x faster, and
  parsing documents with no matches is about 14% faster. ([#94], [#95])
- Tag names are resolved once per tag to a one-byte id that drives parser,
  text, and selector lookups. ([#96])
- Saved elements are stored as columns of 32-bit spans, one per saved field,
  and a column exists only if some query saves that field. Attributes are two
  columns of key and value spans, `class` and `id` included, so an element
  without attributes stores nothing for them. ([#100])
- A dropped `Store` keeps its allocations, up to 32 MiB, for the next parse on
  the same thread, so parsing many documents in a loop no longer allocates
  and page-faults a fresh store each time. ([#100])
- Tags that end right after their name, such as `<div>`, skip attribute
  parsing in every parse, and rarely taken paths leave the parse loop. On
  documents of empty `div`s, parsing is about 16% faster. ([#102])
- Documents the adaptive policy indexes ahead are now indexed with a SIMD
  structural tape (via simdlex) instead of the full-document event index,
  which is removed. The tape records every `<`, `>`, and quote, and is built
  lazily in windows, so a parse that stops early only indexes what it read.
  Dense documents keep the rolling indexer. On a 409 KB prose page, finding
  every link is about 1.9× faster. ([#115])
- Python wheels and Node addons are built with profile-guided optimization,
  trained through each binding on `crates/bindings/pgo`. On the binding
  performance gate's workloads, Python parses 19–22% faster and Node 7–12%
  faster. Every wheel is now built on a native runner. ([#117])

### Fixed

- End tags with attributes, such as `</div class=x>`, close their element.
  ([#92], [#96])
- A `/` ends a tag name, so `<x/y>` opens `x` and `</x/y>` closes it.
  ([#96])
- Elements left open when the input ends right after a tag, optionally
  followed by whitespace, keep their saved content. ([#93], [#96])
- Queries with the same selector string keep separate results; `Store::get`
  returns the first query with that selector. ([#91], [#97])
- Python and Node elements could read freed memory after their `Store` was
  garbage collected. Elements now keep the parsed HTML and queries alive.
  ([#50], [#51])

## [0.1.0] - 2026-10-01

Follows 0.0.21. The minor version moves because of the breaking text changes below.

### Breaking

- **`text` replaces `text_content` and produces different output.** See
  [Migrating from `text_content`](#migrating-from-text_content). ([#56], [#59], [#60])
- `Save::all()` now captures `inner_html`, `raw_text`, and `text`, which does
  more work than the previous two-field `Save::all()`. Use `Save::only_text()`
  or `Save::only_raw_text()` when you need one representation.

### Added

- Adjacent (`+`) and general (`~`) sibling combinators, evaluated in a single
  pass without retaining earlier siblings. ([#40], [#41])
- Universal selectors, selector lists, attribute value flags (`i`, `s`),
  `:not()`, `:is()`, `:where()`, child and type ordinals (including
  `:nth-child(An+B of S)`), `:root`, and `:scope`. ([#64], [#65], [#66])
- `raw_text` / `rawText`: source-preserving descendant text. ([#56])
- Normalized `text` with HTML character reference decoding, generated from the
  WHATWG entity table. ([#55], [#56])
- `Save::name_only()` for capturing element names without attributes. ([#35])

### Changed

- Only the attributes that active queries need are parsed. ([#34], [#35])
- SIMD tag indexing that adapts to document density and query lifetime. ([#36], [#37], [#38])
- Finished query runners are skipped during dispatch. ([#42], [#45])
- Parsing without text capture no longer pays for text-capture code, and text
  extraction overhead is lower. ([#57], [#68])
- The crates declare a minimum supported Rust version of 1.88. ([#73])
- Releases publish through trusted publishing, so all npm packages, including
  the per-platform binaries, carry provenance. ([#79])

### Deprecated

- Rust: `Save::only_text_content()` and `Element::text_content()`.
- Python: `Save.only_text_content()`, `Save(text_content=...)`, and
  `Element.text_content`. These emit `DeprecationWarning`.
- Node: the `textContent` save option and `Element.textContent`.

The aliases return the new normalized `text`. They do not restore the old
output.

### Fixed

- Selector edge cases and Python error handling. ([#25])

### Migrating from `text_content`

`text` is not a renamed `text_content`. It decodes character references, omits
`script`, `style`, `template`, and `[hidden]` subtrees, inserts structural line
and table-cell boundaries, and applies different whitespace rules. Code that
compares, hashes, indexes, or persists extracted text should treat this as a
data migration.

For example, extracting the text of `main` from:

```html
<main><p>A&nbsp;&amp; B</p><div hidden>secret</div><p>C</p></main>
```

```text
old text_content: "A&nbsp;&amp; B secret C"
new text:         "A\u{00A0}& B\nC"
```

| Before | After |
| ------ | ----- |
| `Save::only_text_content()` | `Save::only_text()` |
| `Save { text_content: true, ... }` | `Save { text: true, ... }` |
| `element.text_content(&store)` | `element.text(&store)` |
| `CapacityOptions::reserve_text_content` | `CapacityOptions::reserve_text` |
| Python `element.text_content` | Python `element.text` |
| Node `element.textContent` | Node `element.text` |
| No raw equivalent | `raw_text` / `rawText` |

`Store::with_capacity` reserves capacity for both text representations by
default. `parse` grows the requested text buffers only after a query matches.

## 0.0.21 and earlier

See the [GitHub releases](https://github.com/zacharyvmm/scah/releases).

[Unreleased]: https://github.com/zacharyvmm/scah/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/zacharyvmm/scah/compare/v0.0.21...v0.1.0
[#25]: https://github.com/zacharyvmm/scah/pull/25
[#34]: https://github.com/zacharyvmm/scah/pull/34
[#35]: https://github.com/zacharyvmm/scah/pull/35
[#36]: https://github.com/zacharyvmm/scah/pull/36
[#37]: https://github.com/zacharyvmm/scah/pull/37
[#38]: https://github.com/zacharyvmm/scah/pull/38
[#40]: https://github.com/zacharyvmm/scah/pull/40
[#41]: https://github.com/zacharyvmm/scah/pull/41
[#42]: https://github.com/zacharyvmm/scah/pull/42
[#45]: https://github.com/zacharyvmm/scah/pull/45
[#55]: https://github.com/zacharyvmm/scah/pull/55
[#56]: https://github.com/zacharyvmm/scah/pull/56
[#57]: https://github.com/zacharyvmm/scah/pull/57
[#59]: https://github.com/zacharyvmm/scah/pull/59
[#60]: https://github.com/zacharyvmm/scah/pull/60
[#64]: https://github.com/zacharyvmm/scah/pull/64
[#65]: https://github.com/zacharyvmm/scah/pull/65
[#66]: https://github.com/zacharyvmm/scah/pull/66
[#68]: https://github.com/zacharyvmm/scah/pull/68
[#73]: https://github.com/zacharyvmm/scah/pull/73
[#79]: https://github.com/zacharyvmm/scah/pull/79
[#94]: https://github.com/zacharyvmm/scah/pull/94
[#95]: https://github.com/zacharyvmm/scah/pull/95
[#92]: https://github.com/zacharyvmm/scah/issues/92
[#93]: https://github.com/zacharyvmm/scah/issues/93
[#96]: https://github.com/zacharyvmm/scah/pull/96
[#91]: https://github.com/zacharyvmm/scah/issues/91
[#97]: https://github.com/zacharyvmm/scah/pull/97
[#98]: https://github.com/zacharyvmm/scah/pull/98
[#100]: https://github.com/zacharyvmm/scah/pull/100
[#102]: https://github.com/zacharyvmm/scah/pull/102
[#112]: https://github.com/zacharyvmm/scah/pull/112
[#113]: https://github.com/zacharyvmm/scah/pull/113
[#115]: https://github.com/zacharyvmm/scah/pull/115
[#50]: https://github.com/zacharyvmm/scah/pull/50
[#51]: https://github.com/zacharyvmm/scah/pull/51
[#48]: https://github.com/zacharyvmm/scah/pull/48
[#49]: https://github.com/zacharyvmm/scah/pull/49
[#53]: https://github.com/zacharyvmm/scah/pull/53
[#117]: https://github.com/zacharyvmm/scah/pull/117
[#122]: https://github.com/zacharyvmm/scah/pull/122
[#123]: https://github.com/zacharyvmm/scah/pull/123
[#124]: https://github.com/zacharyvmm/scah/pull/124
[#125]: https://github.com/zacharyvmm/scah/pull/125
