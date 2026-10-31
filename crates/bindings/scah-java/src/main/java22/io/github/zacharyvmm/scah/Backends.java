package io.github.zacharyvmm.scah;

/**
 * Picks the backend that calls the native library: on Java 22 and newer, the Foreign Function
 * and Memory API, unless the {@code scah.backend} system property asks for {@code jni}.
 */
final class Backends {
    private Backends() {}

    static Backend load() {
        String backend = System.getProperty("scah.backend", "");
        if (backend.isEmpty()) {
            backend = "panama";
        }
        switch (backend) {
            case "panama":
                return new PanamaBackend(NativeLibrary.locate("scah_c"));
            case "jni":
                return new JniBackend();
            default:
                throw new IllegalArgumentException(
                        "scah.backend must be panama or jni, not " + backend);
        }
    }
}
