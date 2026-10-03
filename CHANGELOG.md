# Changelog

All notable changes to scah are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions apply to the
Rust crates and the Python and npm packages together.

## [Unreleased]

### Changed

- Tag names resolve to a one-byte id once per tag, and each opening tag scans a
  single contiguous index of active query cursors instead of walking every
  query. Parsing with 16 or more queries is 14–22% faster.

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
