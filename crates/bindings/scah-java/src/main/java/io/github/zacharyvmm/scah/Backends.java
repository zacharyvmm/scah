package io.github.zacharyvmm.scah;

/**
 * Picks the backend that calls the native library.
 *
 * <p>Java 22 and newer load the multi-release override of this class, which uses the
 * Foreign Function and Memory API.
 */
final class Backends {
    private Backends() {}

    static Backend load() {
        throw new UnsupportedOperationException(
                "scah needs Java 22 or newer; this is Java " + Runtime.version().feature());
    }
}
