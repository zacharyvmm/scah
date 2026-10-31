package io.github.zacharyvmm.scah;

/**
 * Picks the backend that calls the native library: JNI, before Java 22.
 *
 * <p>Java 22 and newer load the multi-release override of this class, which prefers the
 * Foreign Function and Memory API.
 */
final class Backends {
    private Backends() {}

    static Backend load() {
        String backend = System.getProperty("scah.backend", "");
        if (backend.isEmpty()) {
            backend = "jni";
        }
        if (!backend.equals("jni")) {
            throw new UnsupportedOperationException(
                    "scah.backend=" + backend + " needs Java 22 or newer; this is Java "
                            + Runtime.version().feature() + ", which supports only scah.backend=jni");
        }
        return new JniBackend();
    }
}
