# Selector support

scah evaluates CSS selectors in a single forward pass, deciding each element
when its start tag is read. It supports the selectors that can be decided at
that point, and it rejects the rest when the query is built. It never runs a
narrowed version of a selector, so a query either matches what a browser would
match or returns an error.

## Supported syntax

| Syntax | Example | Notes |
|--------|---------|-------|
| Type | `a`, `div` | |
| ID | `#main` | |
| Class | `.card`, `.card.featured` | |
| Universal | `*`, `*.card` | |
| Descendant combinator | `main section a` | |
| Child combinator | `main > section` | |
| Next-sibling combinator | `h1 + p` | Evaluated without keeping earlier siblings |
| Subsequent-sibling combinator | `h1 ~ p` | Evaluated without keeping earlier siblings |
| Attribute presence | `a[href]` | |
| Attribute value | `[lang="en"]`, `[href^="https:"]`, `[src$=".png"]`, `[class*="btn"]` | Exact, prefix, suffix, substring |
| Attribute case flags | `[type="SUBMIT" i]`, `[data-kind="Foo" s]` | ASCII case-insensitive or case-sensitive; the flag itself is case-insensitive |
| Selector lists | `h1, h2`, `main > h1, main > h2` | |
| Negation | `a:not(.external)` | Arguments must be compound selectors |
| Matches-any | `:is(h1, h2) > a`, `:where(.a, .b)` | Arguments must be compound selectors |
| Child ordinals | `li:first-child`, `tr:nth-child(2n+1)`, `:nth-child(odd)` | |
| Type ordinals | `p:first-of-type`, `td:nth-of-type(3)` | |
| Filtered ordinals | `li:nth-child(2 of .result)` | `of S` must be a list of compound selectors |
| Root | `:root` | |
| Scope | `:scope > a` | In nested queries, where `:scope` is the parent match |

A *compound selector* is a sequence of simple selectors with no combinator,
such as `a.external[href]`.

## Rejected selectors

### Selectors that depend on what comes later

These need to see an element's descendants or later siblings, which a single
forward pass has not read when the element opens:

- `:has()`
- `:empty`
- `:last-child`, `:last-of-type`, `:only-child`, `:only-of-type`
- `:nth-last-child()`, `:nth-last-of-type()`

### Selectors scah does not model

These fail closed, even inside nested `:not()`, `:is()`, or `:where()`:

- Combinators inside `:not()`, `:is()`, `:where()`, or `of S`, as in
  `:is(main a)`.
- CSS escapes (`\`), comments (`/* */`), the nesting selector `&`, NUL
  characters, and namespaces (`ns|a`, `[ns|attr]`).
- Escaped or non-ASCII identifiers, including attribute names, and escaped
  attribute values such as `[title="a\"b"]`.
- Out-of-range `An+B` numbers.
- Pseudo-elements, state pseudo-classes such as `:hover` or `:checked`, and
  any pseudo-class not listed above.

### Forgiving and strict selector lists

`:is()` and `:where()` are *forgiving*, as CSS specifies: an alternative that
is invalid CSS is dropped, so `div:is(.card, .bad])` matches `div.card`. An
alternative that is valid CSS but that scah cannot evaluate still rejects the
whole selector, because dropping it would match fewer elements than a browser.

`:not()` and `of S` lists are *strict*: any invalid or unsupported alternative
rejects the selector.

### Limits

- Selector lists inside functional pseudo-classes nest at most
  `MAX_SELECTOR_NESTING_DEPTH` (32) levels deep.
- `:scope` is supported only as a standalone leading anchor, as in
  `:scope > a`. A trailing `:scope` or a compound such as `:scope.card` is
  rejected.

## Errors

A rejected selector fails when the query is built. In Rust that is a
`SelectorParseError` from `Query::all`, `Query::first`, or the builder
methods, and a compile error from `query!`:

```rust
use scah::{Query, Save};

let err = Query::all("li:last-child", Save::none()).unwrap_err();
assert_eq!(err.position(), 2);
println!("{err}"); // "unsupported pseudo-class at byte 2"
```

The Python and JavaScript bindings check selectors when `build()` is called:
Python raises `ValueError`, and JavaScript throws an `Error`.

## Nested queries

A query added with `.then()` matches the descendants of each match of its
parent:

- A leading `>` is relative to the parent match, so `> a` selects the
  parent's child links; `:scope > a` is the same. Without it, the selector
  matches any descendant of the parent.
- A leading `+` or `~` is an error: nested queries only see what is inside
  their parent.
- `first()` keeps the first match within each parent match.
- An element matched under several parent matches is listed under each of
  them.
