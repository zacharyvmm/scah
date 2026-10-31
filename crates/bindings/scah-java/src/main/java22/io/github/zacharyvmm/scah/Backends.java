package io.github.zacharyvmm.scah;

/**
 * Picks the backend that calls the native library: on Java 22 and newer, the Foreign Function
 * and Memory API.
 */
final class Backends {
    private Backends() {}

    static Backend load() {
        return new PanamaBackend(NativeLibrary.locate("scah_c"));
    }
}
