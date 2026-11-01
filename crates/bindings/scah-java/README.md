# scah for Java

**Fast HTML extraction with CSS selectors. One pass, no DOM.**

Java bindings for [scah](https://github.com/zacharyvmm/scah). scah pulls the
elements you select out of an HTML document in a single forward pass: it
streams the document through a native Rust parser once and keeps only the
matches. There is no DOM to build, so it is fast on large pages: finding
every link in the 15 MB WHATWG HTML specification is about 3.5× faster than
jsoup.

```xml
<dependency>
  <groupId>io.github.zacharyvmm</groupId>
  <artifactId>scah</artifactId>
  <version>0.2.0-dev</version>
</dependency>
```

The jar runs on **Java 11 and newer**. On Java 22 and newer it calls the
native library through the Foreign Function and Memory API; before, through
JNI. It ships both libraries for Linux (glibc 2.17+ and musl), macOS, and
Windows, on x64 and arm64.

On Java 22 and newer, run with `--enable-native-access=ALL-UNNAMED` (or the
name of your module) to allow the native calls. Without it, the JVM prints a
warning when scah first loads.

## Usage

```java
import io.github.zacharyvmm.scah.*;

String html = """
    <main>
      <section><a href="/one">One</a><div><a href="/two">Two</a></div></section>
    </main>
    """;

try (Query query = Query.all("main > section", Save.none())
            .then(Query.all("> a[href]", Save.onlyText()), Query.all("div a", Save.onlyText()))
            .build();
        Store store = Store.parse(html, query)) {
    for (Element section : store.get("main > section")) {
        for (Element link : section.get("> a[href]")) {
            System.out.println(link.text().orElse("") + " " + link.attribute("href").orElse(""));
            // One /one
        }
    }
}
```

`Save` controls what is captured for each match: `Save.all()`,
`Save.onlyInnerHtml()`, `Save.onlyRawText()`, `Save.onlyText()`,
`Save.none()` (attributes only), `Save.nameOnly()`, or any combination with the
`with` methods, such as `Save.nameOnly().withText(true)`. Each `Element`
exposes `name()`, `id()`, `className()`, `attributes()`, `innerHtml()`,
`rawText()`, `text()`, `attribute(name)`, and `get(selector)` for the results
of chained sections and nested queries. Fields the query did not save are
`Optional.empty()`.

`Store.get(selector)` returns the results of the first query built with that
selector; `Store.queryResults(index)` picks a query by its position in
`Store.parse`. `Store.parse` also takes UTF-8 bytes, which skips encoding the
document.

## Resources and threads

`Query` and `Store` own native memory. Close them to free it right away;
otherwise it is freed once they become unreachable. A store copies the HTML
and keeps what it needs of its queries, so a query may be closed as soon as
`Store.parse` returns, and reused for any number of parses until then. Reading
an element of a closed store throws `IllegalStateException`, even when another
thread closes it mid-read.

Queries and stores are immutable and may be used from several threads at once.
`QueryBuilder`s are immutable too: each call returns a new builder, so a shared
prefix can be extended in several ways.

Failures throw `ScahException`, whose `status()` says why, such as
`ScahStatus.INVALID_SELECTOR` from `build()`.

## Native library

The jar extracts the library for the current platform into
`java.io.tmpdir/scah-<checksum>/`, once per version, shared by every JVM. Set
`-Dscah.library.cache=<dir>` to extract it elsewhere, or
`-Dscah.library.dir=<dir>` to load the libraries from a directory instead:
`libscah_c` for the Foreign Function and Memory API, `libscah_jni` for JNI.

`-Dscah.backend=jni` selects JNI on Java 22 and newer too. The Foreign
Function and Memory API reads element fields several times faster, while JNI
copies UTF-8 byte arrays once less when parsing.

See [Selector support](https://github.com/zacharyvmm/scah/blob/main/docs/selectors.md)
and [Text extraction](https://github.com/zacharyvmm/scah/blob/main/docs/text-extraction.md)
for the selectors scah supports and what each text field contains.

## Benchmarks

Measured with JMH on Java 25. `scah` uses the Foreign Function and Memory
API; `scah (JNI)` the JNI backend. In the first-match scenario, jsoup's
`StreamParser` and Jericho stop reading at the match, while most of scah's time
goes to encoding the whole `String` as UTF-8 before parsing.

<!-- benchmarks:java -->
| Library | WHATWG spec | Nested (all) | Flat (all) | Flat (first) |
| :--- | ---: | ---: | ---: | ---: |
| **scah** | **71.0 ms** | **25.5 ms** | **6.74 ms** | **76.1 µs** |
| scah (JNI) | 90.7 ms (1.28×) | 38.2 ms (1.50×) | 9.97 ms (1.48×) | 81.2 µs (1.07×) |
| jsoup | 252 ms (3.54×) | 55.6 ms (2.18×) | 22.0 ms (3.26×) | 16.5 ms (216×) |
| jsoup (StreamParser) | 308 ms (4.34×) | 58.2 ms (2.28×) | 28.7 ms (4.25×) | 5.24 µs (0.07×) |
| Jericho | 383 ms (5.40×) | 75.6 ms (2.96×) | 22.7 ms (3.37×) | 2.91 µs (0.04×) |
| HtmlCleaner | 1.36 s (19.1×) | 225 ms (8.80×) | 98.3 ms (14.6×) | 66.2 ms (870×) |
| Jodd Jerry | 1.63 s (22.9×) | 70.2 ms (2.75×) | 45.8 ms (6.78×) | 44.9 ms (590×) |

Mean time per parse and query; lower is better. Multipliers are relative to scah. Synthetic inputs use 10,000 elements. Measured 2026-10-31; raw data and run details in [`benches/results/java`](https://github.com/zacharyvmm/scah/tree/main/benches/results/java).
<!-- /benchmarks:java -->

## Development

The public API and the JNI backend, whose native half is
[`crates/bindings/scah-jni`](../scah-jni), target Java 11. The backend that
calls the C ABI in [`crates/bindings/scah-c`](../scah-c) lives in
`src/main/java22` and is packed as a multi-release class. The JVM only loads
such classes from a jar, so the tests run after packaging, against the jar.
Build the libraries and run the tests with JDK 22 or newer:

```bash
cargo build -p scah-c -p scah-jni
./mvnw verify                      # Foreign Function and Memory API
./mvnw verify -Dscah.backend=jni   # JNI
```

To run them on an older JDK, which uses JNI, point `scah.test.jvm` at its
`java` and drop the native-access option:
`./mvnw verify -Dscah.test.jvm=$JDK11/bin/java -Dscah.test.argLine=`.

The tests load `target/debug`'s libraries; pass `-Dscah.library.dir=<dir>` to
test others. To pack the libraries that
[`crates/bindings/native/build.sh`](../native/build.sh) built, and test the
jar's own copy:

```bash
./mvnw verify -Dscah.native.dir=$PWD/../../../target/native/out -Dscah.library.dir=
```

## License

[MIT](https://github.com/zacharyvmm/scah/blob/main/LICENSE)
