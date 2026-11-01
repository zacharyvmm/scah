# scah for .NET

**Fast HTML extraction with CSS selectors. One pass, no DOM.**

.NET bindings for [scah](https://github.com/zacharyvmm/scah). scah pulls the
elements you select out of an HTML document in a single forward pass: it
streams the document through a native Rust parser once and keeps only the
matches. There is no DOM to build, so it is fast on large pages: finding
every link in the 15 MB WHATWG HTML specification is about 15× faster than
AngleSharp and 25× faster than HtmlAgilityPack.

```bash
dotnet add package Scah
```

The package targets .NET 8 and newer and ships the native library for Linux
(glibc 2.17+ and musl), macOS, and Windows, on x64 and arm64. It is
trimming- and Native AOT-compatible.

## Usage

```csharp
using Scah;

const string Html = """
    <main>
      <section><a href="/one">One</a><div><a href="/two">Two</a></div></section>
    </main>
    """;

using Query query = Query.All("main > section", Save.None)
    .Then(
        Query.All("> a[href]", Save.OnlyText),
        Query.All("div a", Save.OnlyText))
    .Build();

using Store store = Store.Parse(Html, query);
foreach (Element section in store.Get("main > section"))
{
    foreach (Element link in section.Get("> a[href]"))
    {
        Console.WriteLine($"{link.Text} {link.GetAttribute("href")}"); // One /one
    }
}
```

`Save` controls what is captured for each match: `Save.All`,
`Save.OnlyInnerHtml`, `Save.OnlyRawText`, `Save.OnlyText`, `Save.None`
(attributes only), `Save.NameOnly`, or any combination with
`new Save { Text = true, Attributes = true }`. Each `Element` exposes `Name`,
`Id`, `ClassName`, `Attributes`, `InnerHtml`, `RawText`, `Text`,
`GetAttribute(name)`, and `Get(selector)` for the results of chained sections
and nested queries. Fields the query did not save are `null`.

`Store.Get(selector)` returns the results of the first query built with that
selector; `Store.QueryResults(index)` picks a query by its position in
`Store.Parse`. `Store.Parse` also takes UTF-8 bytes (`ReadOnlySpan<byte>`),
which skips transcoding the document.

## Resources and threads

`Query` and `Store` own native memory. Dispose them to free it right away;
otherwise the finalizer frees it. A store copies the HTML and keeps what it
needs of its queries, so a query may be disposed as soon as `Store.Parse`
returns, and reused for any number of parses until then. Reading an element of
a disposed store throws `ObjectDisposedException`.

Queries and stores are immutable and may be used from several threads at once.
`QueryBuilder`s are immutable too: each call returns a new builder, so a shared
prefix can be extended in several ways.

Failures throw `ScahException`, whose `Status` says why, such as
`ScahStatus.InvalidSelector` from `Build()`.

See [Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md)
and [Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md)
for the selectors scah supports and what each text field contains.

## Benchmarks

Measured with BenchmarkDotNet on .NET 10. `HtmlAgilityPack` queries with
XPath, its own query language; `HtmlAgilityPack + Fizzler` with CSS selectors.

<!-- benchmarks:csharp -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **63.4 ms** | **20.1 ms** | **5.16 ms** | **56.6 µs** |
| AngleSharp | 935 ms (14.7×) | 176 ms (8.72×) | 75.0 ms (14.5×) | 62.1 ms (1,097×) |
| HtmlAgilityPack | 1.62 s (25.5×) | 325 ms (16.1×) | 132 ms (25.5×) | 96.9 ms (1,710×) |
| HtmlAgilityPack + Fizzler | 1.68 s (26.4×) | 263 ms (13.0×) | 135 ms (26.2×) | 97.2 ms (1,717×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-31; raw data and run details in [`benches/results/csharp`](https://github.com/zacharyvmm/scah/tree/main/benches/results/csharp).
<!-- /benchmarks:csharp -->

## Development

The binding calls the C ABI in [`crates/bindings/scah-c`](../scah-c) through
`LibraryImport`. Build it and run the tests from this directory:

```bash
cargo build -p scah-c
dotnet test Scah.Tests
```

The tests load `target/debug`'s library; pass
`-p:ScahNativeLibraryDir=<dir>` to test another. To pack the libraries that
[`crates/bindings/native/build.sh`](../native/build.sh) built:

```bash
dotnet pack Scah -c Release -p:ScahNativeDir=$PWD/../../../target/native/out
```

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
