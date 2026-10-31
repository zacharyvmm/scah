# scah for Java

> CSS selectors meet streaming HTML parsing. Extract exactly what you select, without building a DOM.

Java bindings for [scah](https://github.com/zacharyvmm/scah), a Rust HTML
extraction library. scah matches CSS selectors in a single forward pass over
the document and stores only the matches.

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
copies the HTML once less when parsing.

See the [main README](https://github.com/zacharyvmm/scah#selector-support) for
supported selectors and text-extraction semantics.

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
