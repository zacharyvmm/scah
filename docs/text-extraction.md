# Text extraction

scah can keep three representations of a matched element's content. Each one
is opt-in through `Save`, because each one costs time and memory to build.

| Rust | Python | JavaScript | Contents |
| ---- | ------ | ---------- | -------- |
| `inner_html()` | `inner_html` | `innerHtml` | The markup between the element's start and end tags, exactly as written |
| `raw_text()` | `raw_text` | `rawText` | The source text of every descendant, with markup removed and nothing else changed |
| `text()` | `text` | `text` | Normalized, human-readable text, described below |

A representation that the query did not save is `None` in Rust and Python and
`null` in JavaScript. One that was saved but is empty is an empty string.

## Example

```html
<main>
  <h1>Prices</h1>
  <p>Tea &amp; cake:&nbsp;£4</p>
  <script>track()</script>
  <div hidden>draft</div>
  <table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>
</main>
```

Without the whitespace between tags, saving `main` with `Save::all()` gives:

| Field | Value |
| ----- | ----- |
| `inner_html` | `<h1>Prices</h1><p>Tea &amp; cake:&nbsp;£4</p><script>track()</script><div hidden>draft</div><table>…</table>` |
| `raw_text` | `"PricesTea &amp; cake:&nbsp;£4track()draftabcd"` |
| `text` | `"Prices\nTea & cake:\u{a0}£4\na\tb\nc\td"` |

## Normalized `text`

`text` is meant for reading, indexing, and comparing content. It is **not**
the browser's `innerText`: scah does not evaluate CSS, so it cannot know what
a stylesheet hides or how it lays elements out. Instead it follows fixed
rules:

- **Hidden content is omitted.** `script`, `style`, `template`, and elements
  with a `hidden` attribute contribute no text and no separators.
- **Blocks break lines.** Block elements, `<br>`, and `<hr>` produce line
  breaks. Table rows are separated by newlines and table cells by tabs.
  Other elements run together inline.
- **Whitespace collapses**, except inside `pre` and `textarea`, which keep
  their whitespace, including literal or decoded nonbreaking spaces. As in
  HTML, a newline right after their start tag is dropped.
- **Character references are decoded**: named references from the WHATWG
  table, numeric references, the legacy forms without a semicolon that HTML
  accepts in text, and the C1 remapping. Invalid code points become U+FFFD.

The block elements are `address`, `article`, `aside`, `blockquote`, `body`,
`caption`, `colgroup`, `dd`, `details`, `dialog`, `div`, `dl`, `dt`,
`fieldset`, `figcaption`, `figure`, `footer`, `form`, `h1` to `h6`, `header`,
`hgroup`, `legend`, `li`, `main`, `menu`, `nav`, `ol`, `p`, `pre`, `search`,
`section`, `summary`, `table`, `tbody`, `tfoot`, `thead`, and `ul`.

## Choosing what to save

| Rust | Python | JavaScript options | Content saved |
| ---- | ------ | ------------------ | ------------- |
| `Save::all()` | `Save.all()` | `{ innerHtml: true, rawText: true, text: true }` | All three |
| `Save::only_text()` | `Save.only_text()` | `{ text: true }` | Normalized text |
| `Save::only_raw_text()` | `Save.only_raw_text()` | `{ rawText: true }` | Raw text |
| `Save::only_inner_html()` | `Save.only_inner_html()` | `{ innerHtml: true }` | Inner HTML |
| `Save::none()` | `Save.none()` | `{}` | None |
| `Save::name_only()` | `Save.name_only()` | `{ attributes: false }` | None, and no attributes |

Every match keeps its element name. All constructors except `name_only` also
keep its id, class, and other attributes; drop them with
`.without_attributes()` in Rust, `Save(attributes=False)` in Python, or
`attributes: false` in JavaScript.

`Save::all()` does the most work. When you only read one representation, save
only that one. When no query saves any text, the Rust function
`parse_without_text_capture` skips text extraction entirely.

## Migrating from `text_content`

`text` replaced `text_content` in 0.1.0, and its output is different: it
decodes references, omits hidden content, and adds structural line breaks.
The deprecated `text_content` names return the new `text`. See the
[0.1.0 changelog](../CHANGELOG.md#migrating-from-text_content) if you store or
compare extracted text.
