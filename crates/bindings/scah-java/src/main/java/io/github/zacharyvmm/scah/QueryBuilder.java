package io.github.zacharyvmm.scah;

import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/**
 * Describes a query: a chain of selector sections, each of which may have nested child
 * queries. {@link #build} compiles it into a {@link Query}.
 *
 * <p>Builders are immutable: {@link #all}, {@link #first}, and {@link #then(QueryBuilder...)}
 * return a new builder, so a shared prefix can be extended in several ways. They hold no native
 * memory.
 */
public final class QueryBuilder {
    private final Step[] steps;

    private QueryBuilder(Step[] steps) {
        this.steps = steps;
    }

    static QueryBuilder start(boolean first, String selector, Save save) {
        return new QueryBuilder(new Step[] {Step.section(first, selector, save)});
    }

    /**
     * Append a section that matches every element selected by {@code selector} inside each match
     * of the last section.
     */
    public QueryBuilder all(String selector, Save save) {
        return append(Step.section(false, selector, save));
    }

    /**
     * Append a section that matches only the first element selected by {@code selector} inside
     * each match of the last section.
     */
    public QueryBuilder first(String selector, Save save) {
        return append(Step.section(true, selector, save));
    }

    /**
     * Nest {@code children} under the last section. Each child runs inside every match of that
     * section; read its results with {@link Element#get}.
     */
    public QueryBuilder then(QueryBuilder... children) {
        QueryBuilder[] copy = children.clone();
        for (QueryBuilder child : copy) {
            Objects.requireNonNull(child, "children");
        }
        return append(Step.children(copy));
    }

    /** Like {@link #then(QueryBuilder...)}. */
    public QueryBuilder then(List<QueryBuilder> children) {
        return then(children.toArray(new QueryBuilder[0]));
    }

    /**
     * Compile the query.
     *
     * @throws ScahException with {@link ScahStatus#INVALID_SELECTOR} when a selector is invalid
     */
    public Query build() {
        Backend backend = Native.backend();
        long builder = toNative(backend);
        try {
            return new Query(backend.build(builder));
        } finally {
            backend.builderFree(builder);
        }
    }

    private QueryBuilder append(Step step) {
        Step[] next = Arrays.copyOf(steps, steps.length + 1);
        next[steps.length] = step;
        return new QueryBuilder(next);
    }

    /** Replay the steps into a native builder, which the caller frees. */
    private long toNative(Backend backend) {
        Step head = steps[0];
        long builder = backend.builderNew(head.first, head.selector, head.save);
        try {
            for (int i = 1; i < steps.length; i++) {
                Step step = steps[i];
                if (step.children != null) {
                    then(backend, builder, step.children);
                } else {
                    backend.builderAppend(builder, step.first, step.selector, step.save);
                }
            }
            return builder;
        } catch (RuntimeException | Error e) {
            backend.builderFree(builder);
            throw e;
        }
    }

    private static void then(Backend backend, long builder, QueryBuilder[] children) {
        // The native builder copies its children, so they are freed right away.
        long[] handles = new long[children.length];
        try {
            for (int i = 0; i < children.length; i++) {
                handles[i] = children[i].toNative(backend);
            }
            backend.builderThen(builder, handles);
        } finally {
            for (long handle : handles) {
                backend.builderFree(handle);
            }
        }
    }

    /** A section to append, or children to nest under the last section. */
    private static final class Step {
        final boolean first;
        final String selector;
        final Save save;
        final QueryBuilder[] children;

        private Step(boolean first, String selector, Save save, QueryBuilder[] children) {
            this.first = first;
            this.selector = selector;
            this.save = save;
            this.children = children;
        }

        static Step section(boolean first, String selector, Save save) {
            return new Step(
                    first,
                    Objects.requireNonNull(selector, "selector"),
                    Objects.requireNonNull(save, "save"),
                    null);
        }

        static Step children(QueryBuilder[] children) {
            return new Step(false, null, null, children);
        }
    }
}
