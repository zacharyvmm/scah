package io.github.zacharyvmm.scah;

import java.lang.ref.Cleaner;
import java.util.List;
import java.util.Objects;

/**
 * The elements that one or more queries matched in a document.
 *
 * <p>A store owns a copy of the HTML and is immutable, so it may be read from several threads
 * at once. Close it to free its memory right away; reading a closed store throws {@link
 * IllegalStateException}. Stores that are never closed are freed once neither they nor any of
 * their elements are reachable.
 */
public final class Store implements AutoCloseable {
    final NativeResource resource;
    private final Cleaner.Cleanable cleanable;

    private Store(long handle) {
        Backend backend = Native.backend();
        resource = new NativeResource(handle, backend::storeFree, "store");
        cleanable = Query.CLEANER.register(this, resource);
    }

    /**
     * Parse {@code html} against {@code queries}.
     *
     * <p>Unpaired surrogates in {@code html} are replaced with U+FFFD, as the Encoding Standard
     * does.
     *
     * @throws ScahException when no queries are given, or the HTML nests too deeply or is 4 GiB
     *     or longer
     * @throws IllegalStateException when a query is closed
     */
    public static Store parse(String html, Query... queries) {
        Objects.requireNonNull(html, "html");
        return parse(queries, (backend, handles) -> backend.parseHtml(encode(backend, html), handles));
    }

    /** UTF-16 units of a string handed to the native library per call. */
    private static final int BLOCK = 8192;

    /**
     * Native HTML holding {@code html} as UTF-8.
     *
     * <p>The library encodes the string a block at a time, straight into the buffer the store
     * will own: one pass over the document, where encoding it here, copying it into native
     * memory, and validating it there would be three. Each block is still in cache when it is
     * encoded, and no copy of the whole document is allocated.
     */
    private static long encode(Backend backend, String html) {
        // Backends reserve the worst case, 3 bytes a unit, before each block. Twice a block of
        // slack keeps that from regrowing an ASCII document's buffer near its end; the library
        // trims what is left over once the string is encoded.
        int length = html.length();
        long encoded = backend.htmlNew(length + 2L * Math.min(length, BLOCK) + 3);
        try {
            char[] block = new char[Math.min(html.length(), BLOCK)];
            // Counted from the remainder, so no index passes the length near Integer.MAX_VALUE.
            for (int start = 0; start < html.length(); ) {
                int count = Math.min(html.length() - start, BLOCK);
                html.getChars(start, start + count, block, 0);
                backend.htmlPush(encoded, block, count);
                start += count;
            }
            return encoded;
        } catch (RuntimeException | Error e) {
            backend.htmlFree(encoded);
            throw e;
        }
    }

    /**
     * Parse UTF-8 {@code html} against {@code queries}.
     *
     * @throws ScahException when the HTML is not valid UTF-8, no queries are given, or the HTML
     *     nests too deeply or is 4 GiB or longer
     * @throws IllegalStateException when a query is closed
     */
    public static Store parse(byte[] html, Query... queries) {
        Objects.requireNonNull(html, "html");
        return parse(queries, (backend, handles) -> backend.parse(html, handles));
    }

    private interface Parser {
        long parse(Backend backend, long[] queries);
    }

    private static Store parse(Query[] queries, Parser parser) {
        Backend backend = Native.backend();
        long[] handles = new long[queries.length];
        int acquired = 0;
        try {
            // Keep every query open while the native call reads it.
            for (; acquired < queries.length; acquired++) {
                Objects.requireNonNull(queries[acquired], "queries");
                handles[acquired] = queries[acquired].resource.acquire();
            }
            return new Store(parser.parse(backend, handles));
        } finally {
            for (int i = 0; i < acquired; i++) {
                queries[i].resource.release();
            }
        }
    }

    /** The number of elements saved, across all queries. */
    public long elementCount() {
        long handle = resource.acquire();
        try {
            return Native.backend().storeLength(handle);
        } finally {
            resource.release();
        }
    }

    /**
     * The elements matched by the top-level section built with {@code selector}, in document
     * order.
     *
     * <p>{@code selector} must be the exact string the section was built with. When several
     * queries share it, the first one's results are returned; use {@link #queryResults} to pick
     * another. The list is empty when nothing matched or no section has that selector.
     */
    public List<Element> get(String selector) {
        Objects.requireNonNull(selector, "selector");
        long handle = resource.acquire();
        try {
            return new ElementList(this, Native.backend().storeGet(handle, selector));
        } finally {
            resource.release();
        }
    }

    /**
     * The elements matched by the query at {@code index} in the array given to {@link
     * #parse(String, Query...)}, in document order.
     *
     * <p>The list is empty when nothing matched or there is no such query.
     */
    public List<Element> queryResults(int index) {
        if (index < 0) {
            throw new IndexOutOfBoundsException("Negative query index " + index);
        }
        long handle = resource.acquire();
        try {
            return new ElementList(this, Native.backend().storeQuery(handle, index));
        } finally {
            resource.release();
        }
    }

    /** Free the store. Its elements can no longer be read. Closing twice does nothing. */
    @Override
    public void close() {
        cleanable.clean();
    }
}
