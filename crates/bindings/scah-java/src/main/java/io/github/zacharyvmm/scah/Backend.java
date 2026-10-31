package io.github.zacharyvmm.scah;

/**
 * The calls the public API makes into the native library.
 *
 * <p>Each method maps to one or a few functions of the C ABI ({@code scah.h}) and passes
 * handles as raw addresses. Callers keep every handle alive for the duration of a call (see
 * {@link NativeResource}). Failures throw {@link ScahException}.
 */
interface Backend {
    /** The fields read by {@link #field}. */
    enum Field {
        ID,
        CLASS,
        INNER_HTML,
        RAW_TEXT,
        TEXT
    }

    /** Start a native builder with one section. The caller frees it. */
    long builderNew(boolean first, String selector, Save save);

    /** Append a section to a native builder. */
    void builderAppend(long builder, boolean first, String selector, Save save);

    /** Nest copies of {@code children} under a native builder's last section. */
    void builderThen(long builder, long[] children);

    /** Free a native builder; 0 is ignored. */
    void builderFree(long builder);

    /** Compile a native builder into a query. */
    long build(long builder);

    /** Free a query. */
    void queryFree(long query);

    /** Parse {@code html} against {@code queries}. */
    long parse(String html, long[] queries);

    /** Parse UTF-8 {@code html} against {@code queries}. */
    long parse(byte[] html, long[] queries);

    /** Free a store. */
    void storeFree(long store);

    /** The number of elements in a store. */
    long storeLength(long store);

    /** The ids matched by a top-level section; empty when there are none. */
    long[] storeGet(long store, String selector);

    /** The ids matched by the query at {@code index}; empty when there are none. */
    long[] storeQuery(long store, long index);

    /** The ids matched under {@code element} by section {@code selector}; empty when there are none. */
    long[] elementGet(long store, long element, String selector);

    /** The element's tag name. */
    String name(long store, long element);

    /** One of the element's optional fields, or null when it is missing. */
    String field(long store, long element, Field field);

    /** The value of the attribute named {@code key}, or null when it is missing. */
    String attribute(long store, long element, String key);

    /** The element's attributes as alternating names and values; a valueless attribute has a null value. */
    String[] attributes(long store, long element);
}
