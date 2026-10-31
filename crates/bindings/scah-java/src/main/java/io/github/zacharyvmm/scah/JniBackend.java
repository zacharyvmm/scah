package io.github.zacharyvmm.scah;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Calls scah through JNI, for Java versions without the Foreign Function and Memory API.
 *
 * <p>The native methods are implemented by the {@code scah_jni} library
 * ({@code crates/bindings/scah-jni}), which links scah itself rather than {@code scah_c}.
 */
final class JniBackend implements Backend {
    /** The version of the native methods this class declares; see PROTOCOL_VERSION in lib.rs. */
    private static final int PROTOCOL_VERSION = 2;

    // Save flags, as in lib.rs.
    private static final int INNER_HTML = 1;
    private static final int RAW_TEXT = 2;
    private static final int TEXT = 4;
    private static final int ATTRIBUTES = 8;

    JniBackend() {
        Path library = NativeLibrary.locate("scah_jni").toAbsolutePath();
        try {
            System.load(library.toString());
        } catch (UnsatisfiedLinkError e) {
            // The JVM binds a JNI library to one classloader. When another one, such as another
            // application in the same container, loaded this jar first, load a private copy.
            if (e.getMessage() == null || !e.getMessage().contains("another classloader")) {
                throw e;
            }
            library = privateCopy(library);
            System.load(library.toString());
        }
        int version = protocolVersion0();
        if (version != PROTOCOL_VERSION) {
            throw new UnsatisfiedLinkError(library + " implements version " + version
                    + " of the native methods, but this binding needs version " + PROTOCOL_VERSION);
        }
    }

    /** A copy of {@code library} in a new directory of the cache, deleted when the JVM exits. */
    private static Path privateCopy(Path library) {
        try {
            Path directory = NativeLibrary.privateDirectory("jni-");
            Path copy = Files.copy(library, directory.resolve(library.getFileName()));
            // Files delete on exit in the reverse order they were registered.
            directory.toFile().deleteOnExit();
            copy.toFile().deleteOnExit();
            return copy;
        } catch (IOException e) {
            throw new UncheckedIOException("Cannot copy " + library + " for this classloader", e);
        }
    }

    private static int flags(Save save) {
        return (save.innerHtml() ? INNER_HTML : 0)
                | (save.rawText() ? RAW_TEXT : 0)
                | (save.text() ? TEXT : 0)
                | (save.attributes() ? ATTRIBUTES : 0);
    }

    @Override
    public long builderNew(boolean first, String selector, Save save) {
        return builderNew0(first, selector, flags(save));
    }

    @Override
    public void builderAppend(long builder, boolean first, String selector, Save save) {
        builderAppend0(builder, first, selector, flags(save));
    }

    @Override
    public void builderThen(long builder, long[] children) {
        builderThen0(builder, children);
    }

    @Override
    public void builderFree(long builder) {
        builderFree0(builder);
    }

    @Override
    public long build(long builder) {
        return build0(builder);
    }

    @Override
    public void queryFree(long query) {
        queryFree0(query);
    }

    @Override
    public long htmlNew(long capacity) {
        return htmlNew0(capacity);
    }

    @Override
    public void htmlPush(long html, char[] units, int length) {
        htmlPush0(html, units, length);
    }

    @Override
    public void htmlFree(long html) {
        htmlFree0(html);
    }

    @Override
    public long parseHtml(long html, long[] queries) {
        return parseHtml0(html, queries);
    }

    @Override
    public long parse(byte[] html, long[] queries) {
        return parse0(html, queries);
    }

    @Override
    public void storeFree(long store) {
        storeFree0(store);
    }

    @Override
    public long storeLength(long store) {
        return storeLength0(store);
    }

    @Override
    public long[] storeGet(long store, String selector) {
        return storeGet0(store, selector);
    }

    @Override
    public long[] storeQuery(long store, long index) {
        return storeQuery0(store, index);
    }

    @Override
    public long[] elementGet(long store, long element, String selector) {
        return elementGet0(store, element, selector);
    }

    @Override
    public String name(long store, long element) {
        return name0(store, element);
    }

    @Override
    public String field(long store, long element, Field field) {
        return field0(store, element, field.ordinal());
    }

    @Override
    public String attribute(long store, long element, String key) {
        return attribute0(store, element, key);
    }

    @Override
    public String[] attributes(long store, long element) {
        return attributes0(store, element);
    }

    private static native int protocolVersion0();

    private static native long builderNew0(boolean first, String selector, int save);

    private static native void builderAppend0(long builder, boolean first, String selector, int save);

    private static native void builderThen0(long builder, long[] children);

    private static native void builderFree0(long builder);

    private static native long build0(long builder);

    private static native void queryFree0(long query);

    private static native long parse0(byte[] html, long[] queries);

    private static native long htmlNew0(long capacity);

    private static native void htmlPush0(long html, char[] units, int length);

    private static native void htmlFree0(long html);

    private static native long parseHtml0(long html, long[] queries);

    private static native void storeFree0(long store);

    private static native long storeLength0(long store);

    private static native long[] storeGet0(long store, String selector);

    private static native long[] storeQuery0(long store, long index);

    private static native long[] elementGet0(long store, long element, String selector);

    private static native String name0(long store, long element);

    private static native String field0(long store, long element, int field);

    private static native String attribute0(long store, long element, String key);

    private static native String[] attributes0(long store, long element);
}
