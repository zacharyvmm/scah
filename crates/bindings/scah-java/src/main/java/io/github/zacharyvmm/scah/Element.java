package io.github.zacharyvmm.scah;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

/**
 * An element a query matched: a row of its {@link Store}.
 *
 * <p>Elements are cheap handles; each method reads the store and returns a new string. An
 * element keeps its store reachable, but reading an element of a closed store throws {@link
 * IllegalStateException}. Fields the query did not save are empty.
 */
public final class Element {
    private final Store store;
    private final long id;

    Element(Store store, long id) {
        this.store = store;
        this.id = id;
    }

    /** The store this element belongs to. */
    public Store store() {
        return store;
    }

    /** The element's row in its store, unique within it. */
    public long index() {
        return id;
    }

    /** The tag name. */
    public String name() {
        long handle = store.resource.acquire();
        try {
            return Native.backend().name(handle, id);
        } finally {
            store.resource.release();
        }
    }

    /** The {@code id} attribute, if attributes were saved. */
    public Optional<String> id() {
        return field(Backend.Field.ID);
    }

    /** The {@code class} attribute, if attributes were saved. */
    public Optional<String> className() {
        return field(Backend.Field.CLASS);
    }

    /** The HTML between the element's tags, if saved. */
    public Optional<String> innerHtml() {
        return field(Backend.Field.INNER_HTML);
    }

    /** The descendant text as written in the source, if saved. */
    public Optional<String> rawText() {
        return field(Backend.Field.RAW_TEXT);
    }

    /** The descendant text with entities decoded and whitespace collapsed, if saved. */
    public Optional<String> text() {
        return field(Backend.Field.TEXT);
    }

    /**
     * The value of the attribute named {@code name}, compared ASCII case-insensitively.
     *
     * <p>Empty when the attribute is absent, has no value (as in {@code <input disabled>}), or
     * attributes were not saved.
     */
    public Optional<String> attribute(String name) {
        Objects.requireNonNull(name, "name");
        long handle = store.resource.acquire();
        try {
            return Optional.ofNullable(Native.backend().attribute(handle, id, name));
        } finally {
            store.resource.release();
        }
    }

    /**
     * The attributes other than {@code id} and {@code class}, in source order, or an empty list
     * when attributes were not saved.
     */
    public List<Attribute> attributes() {
        String[] pairs;
        long handle = store.resource.acquire();
        try {
            pairs = Native.backend().attributes(handle, id);
        } finally {
            store.resource.release();
        }
        if (pairs.length == 0) {
            return Collections.emptyList();
        }
        List<Attribute> attributes = new ArrayList<>(pairs.length / 2);
        for (int i = 0; i < pairs.length; i += 2) {
            attributes.add(new Attribute(pairs[i], pairs[i + 1]));
        }
        return Collections.unmodifiableList(attributes);
    }

    /**
     * The elements matched under this one by the section built with {@code selector}: the
     * section chained after this element's with {@link QueryBuilder#all} or {@link
     * QueryBuilder#first}, or a child query nested with {@link QueryBuilder#then(QueryBuilder...)}.
     *
     * <p>The list is empty when nothing matched or no such section has that selector.
     */
    public List<Element> get(String selector) {
        Objects.requireNonNull(selector, "selector");
        long handle = store.resource.acquire();
        try {
            return new ElementList(store, Native.backend().elementGet(handle, id, selector));
        } finally {
            store.resource.release();
        }
    }

    private Optional<String> field(Backend.Field field) {
        long handle = store.resource.acquire();
        try {
            return Optional.ofNullable(Native.backend().field(handle, id, field));
        } finally {
            store.resource.release();
        }
    }

    /** Whether {@code other} is the same row of the same store. */
    @Override
    public boolean equals(Object other) {
        return other instanceof Element
                && ((Element) other).store == store
                && ((Element) other).id == id;
    }

    @Override
    public int hashCode() {
        return 31 * System.identityHashCode(store) + Long.hashCode(id);
    }

    @Override
    public String toString() {
        return "Element[index=" + id + "]";
    }
}
