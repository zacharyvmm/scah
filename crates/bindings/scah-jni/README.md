# scah-jni

The native half of the Java binding's JNI backend, used on Java 11 to 21 or
with `-Dscah.backend=jni`. It implements the native methods of
`io.github.zacharyvmm.scah.JniBackend` in
[`crates/bindings/scah-java`](../scah-java) and, like the Python and Node
bindings, calls scah through `scah-ffi` rather than the C ABI.

```bash
cargo build -p scah-jni
```

The Java binding's tests exercise it; see its README.
