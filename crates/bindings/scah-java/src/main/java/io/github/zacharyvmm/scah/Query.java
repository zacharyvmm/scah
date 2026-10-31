package io.github.zacharyvmm.scah;

import java.lang.ref.Cleaner;

/**
 * A compiled, immutable query, ready to pass to {@link Store#parse(String, Query...)}.
 *
 * <p>Queries may be shared across threads and reused for any number of parses. A store keeps
 * what it needs of its queries, so a query may be closed as soon as parsing returns. Queries
 * that are never closed are freed once they become unreachable.
 */
public final class Query implements AutoCloseable {
    static final Cleaner CLEANER = Cleaner.create();

    final NativeResource resource;
    private final Cleaner.Cleanable cleanable;

    Query(long handle) {
        Backend backend = Native.backend();
        resource = new NativeResource(handle, backend::queryFree, "query");
        cleanable = CLEANER.register(this, resource);
    }

    /** Start a query whose first section matches every element selected by {@code selector}. */
    public static QueryBuilder all(String selector, Save save) {
        return QueryBuilder.start(false, selector, save);
    }

    /**
     * Start a query whose first section matches only the first element selected by {@code
     * selector}, which lets parsing stop early.
     */
    public static QueryBuilder first(String selector, Save save) {
        return QueryBuilder.start(true, selector, save);
    }

    /** Free the compiled query. Stores parsed with it stay valid. Closing twice does nothing. */
    @Override
    public void close() {
        cleanable.clean();
    }
}
