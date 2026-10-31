package io.github.zacharyvmm.scah;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.Locale;
import org.junit.jupiter.api.Test;

class BackendTest {
    @Test
    void usesTheRequestedBackend() {
        String requested = System.getProperty("scah.backend", "");
        String expected = !requested.isEmpty() ? requested : Runtime.version().feature() >= 22 ? "panama" : "jni";
        String loaded = Native.backend().getClass().getSimpleName().toLowerCase(Locale.ROOT);
        assertEquals(expected + "backend", loaded);
    }
}
