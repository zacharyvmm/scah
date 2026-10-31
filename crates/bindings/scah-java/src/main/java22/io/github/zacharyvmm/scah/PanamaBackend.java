package io.github.zacharyvmm.scah;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_BOOLEAN;
import static java.lang.foreign.ValueLayout.JAVA_BYTE;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SegmentAllocator;
import java.lang.foreign.SymbolLookup;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;

/**
 * Calls the C ABI through the Foreign Function and Memory API.
 *
 * <p>Out-parameters and short input strings live in a per-thread scratch area, so most calls
 * allocate no native memory. Calls that read a single element are linked as {@linkplain
 * Linker.Option#critical critical}, which skips the thread-state transition; they run in
 * constant time or in time linear in the element's attributes. Calls that build a result list
 * take time linear in its length, so they stay ordinary and let the JVM reach safepoints.
 */
// Every restricted call is the point of this class: it links and reads the native library.
@SuppressWarnings("restricted")
final class PanamaBackend implements Backend {
    /** {@code ScahStringView}: a pointer and a {@code size_t} length. */
    private static final MemoryLayout STRING_VIEW =
            MemoryLayout.structLayout(ADDRESS.withName("data"), JAVA_LONG.withName("len"));

    /** {@code ScahOptionalStringView}: a C {@code bool}, padding, and a string view. */
    private static final MemoryLayout OPTIONAL_VIEW = MemoryLayout.structLayout(
            JAVA_BOOLEAN.withName("is_some"),
            MemoryLayout.paddingLayout(7),
            STRING_VIEW.withName("value"));

    /** {@code ScahSave}: four C {@code bool}s. */
    private static final MemoryLayout SAVE = MemoryLayout.structLayout(
            JAVA_BOOLEAN.withName("inner_html"),
            JAVA_BOOLEAN.withName("raw_text"),
            JAVA_BOOLEAN.withName("text"),
            JAVA_BOOLEAN.withName("attributes"));

    /** The C ABI version these declarations match. */
    private static final int ABI_VERSION = 1;

    private final MethodHandle errorMessage;
    private final MethodHandle errorFree;
    private final MethodHandle builderNewAll;
    private final MethodHandle builderNewFirst;
    private final MethodHandle builderAll;
    private final MethodHandle builderFirst;
    private final MethodHandle builderThen;
    private final MethodHandle builderFree;
    private final MethodHandle queryBuild;
    private final MethodHandle queryFree;
    private final MethodHandle parse;
    private final MethodHandle htmlNew;
    private final MethodHandle htmlReserve;
    private final MethodHandle htmlPushUtf16;
    private final MethodHandle htmlFree;
    private final MethodHandle parseHtml;
    private final MethodHandle storeFree;
    private final MethodHandle storeLen;
    private final MethodHandle storeGet;
    private final MethodHandle storeQuery;
    private final MethodHandle elementGet;
    private final MethodHandle listLen;
    private final MethodHandle listIds;
    private final MethodHandle listFree;
    private final MethodHandle elementName;
    private final MethodHandle[] fields;
    private final MethodHandle elementAttribute;
    private final MethodHandle attributeCount;
    private final MethodHandle attributeAt;

    private final ThreadLocal<Scratch> scratch = ThreadLocal.withInitial(Scratch::new);

    PanamaBackend(Path library) {
        if (Linker.nativeLinker().canonicalLayouts().get("size_t").byteSize() != 8) {
            throw new UnsupportedOperationException("scah needs a 64-bit platform");
        }
        SymbolLookup symbols = SymbolLookup.libraryLookup(library, Arena.global());
        Binder bind = new Binder(symbols);

        int abi;
        try {
            abi = (int) bind.function("scah_abi_version", FunctionDescriptor.of(JAVA_INT)).invokeExact();
        } catch (Throwable e) {
            throw rethrow(e);
        }
        if (abi != ABI_VERSION) {
            throw new UnsatisfiedLinkError(library + " has ABI version " + Integer.toUnsignedString(abi)
                    + ", but this binding needs version " + ABI_VERSION);
        }

        errorMessage = bind.reader("scah_error_message", FunctionDescriptor.of(STRING_VIEW, ADDRESS));
        errorFree = bind.reader("scah_error_free", FunctionDescriptor.ofVoid(ADDRESS));

        FunctionDescriptor newBuilder = FunctionDescriptor.of(JAVA_INT, STRING_VIEW, SAVE, ADDRESS, ADDRESS);
        builderNewAll = bind.function("scah_query_builder_new_all", newBuilder);
        builderNewFirst = bind.function("scah_query_builder_new_first", newBuilder);
        FunctionDescriptor append = FunctionDescriptor.of(JAVA_INT, ADDRESS, STRING_VIEW, SAVE, ADDRESS);
        builderAll = bind.function("scah_query_builder_all", append);
        builderFirst = bind.function("scah_query_builder_first", append);
        builderThen = bind.function(
                "scah_query_builder_then", FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS));
        builderFree = bind.function("scah_query_builder_free", FunctionDescriptor.ofVoid(ADDRESS));
        queryBuild = bind.function("scah_query_build", FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, ADDRESS));
        queryFree = bind.function("scah_query_free", FunctionDescriptor.ofVoid(ADDRESS));

        parse = bind.function(
                "scah_parse", FunctionDescriptor.of(JAVA_INT, STRING_VIEW, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS));
        htmlNew = bind.function(
                "scah_html_new", FunctionDescriptor.of(JAVA_INT, JAVA_LONG, JAVA_BOOLEAN, ADDRESS, ADDRESS));
        htmlReserve = bind.function(
                "scah_html_reserve", FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, ADDRESS));
        htmlPushUtf16 = bind.arrayReader(
                "scah_html_push_utf16", FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS));
        htmlFree = bind.function("scah_html_free", FunctionDescriptor.ofVoid(ADDRESS));
        parseHtml = bind.function(
                "scah_parse_html", FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS));
        storeFree = bind.function("scah_store_free", FunctionDescriptor.ofVoid(ADDRESS));
        storeLen = bind.reader("scah_store_len", FunctionDescriptor.of(JAVA_LONG, ADDRESS));
        storeGet = bind.function(
                "scah_store_get", FunctionDescriptor.of(JAVA_INT, ADDRESS, STRING_VIEW, ADDRESS, ADDRESS));
        storeQuery = bind.function(
                "scah_store_query", FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS));
        elementGet = bind.function(
                "scah_element_get",
                FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, STRING_VIEW, ADDRESS, ADDRESS));
        listLen = bind.reader("scah_element_list_len", FunctionDescriptor.of(JAVA_LONG, ADDRESS));
        listIds = bind.reader("scah_element_list_ids", FunctionDescriptor.of(ADDRESS, ADDRESS));
        listFree = bind.reader("scah_element_list_free", FunctionDescriptor.ofVoid(ADDRESS));

        FunctionDescriptor getter = FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS);
        elementName = bind.reader("scah_element_name", getter);
        fields = new MethodHandle[Field.values().length];
        fields[Field.ID.ordinal()] = bind.reader("scah_element_id", getter);
        fields[Field.CLASS.ordinal()] = bind.reader("scah_element_class_name", getter);
        fields[Field.INNER_HTML.ordinal()] = bind.reader("scah_element_inner_html", getter);
        fields[Field.RAW_TEXT.ordinal()] = bind.reader("scah_element_raw_text", getter);
        fields[Field.TEXT.ordinal()] = bind.reader("scah_element_text", getter);
        elementAttribute = bind.reader(
                "scah_element_attribute",
                FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, STRING_VIEW, ADDRESS, ADDRESS));
        attributeCount = bind.reader("scah_element_attribute_count", getter);
        attributeAt = bind.reader(
                "scah_element_attribute_at",
                FunctionDescriptor.of(JAVA_INT, ADDRESS, JAVA_LONG, JAVA_LONG, ADDRESS, ADDRESS, ADDRESS));
    }

    @Override
    public long builderNew(boolean first, String selector, Save save) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment view = s.view(arena, selector);
            MethodHandle handle = first ? builderNewFirst : builderNewAll;
            check((int) handle.invokeExact(view, s.save(save), s.out, s.error));
            return s.out.get(ADDRESS, 0).address();
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void builderAppend(long builder, boolean first, String selector, Save save) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment view = s.view(arena, selector);
            MethodHandle handle = first ? builderFirst : builderAll;
            check((int) handle.invokeExact(MemorySegment.ofAddress(builder), view, s.save(save), s.error));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void builderThen(long builder, long[] children) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment list = arena.allocateFrom(JAVA_LONG, children);
            check((int) builderThen.invokeExact(
                    MemorySegment.ofAddress(builder), list, (long) children.length, s.error));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void builderFree(long builder) {
        try {
            builderFree.invokeExact(MemorySegment.ofAddress(builder));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long build(long builder) {
        Scratch s = scratch.get();
        try {
            check((int) queryBuild.invokeExact(MemorySegment.ofAddress(builder), s.out, s.error));
            return s.out.get(ADDRESS, 0).address();
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void queryFree(long query) {
        try {
            queryFree.invokeExact(MemorySegment.ofAddress(query));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long htmlNew(long capacity) {
        Scratch s = scratch.get();
        try {
            check((int) htmlNew.invokeExact(capacity, false, s.out, s.error));
            return s.out.get(ADDRESS, 0).address();
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void htmlPush(long html, char[] units, int length) {
        MemorySegment encoded = MemorySegment.ofAddress(html);
        MemorySegment error = scratch.get().error;
        try {
            // Grow the buffer in an ordinary call: a reallocation copies the whole document so far,
            // and the push below holds off safepoints.
            check((int) htmlReserve.invokeExact(encoded, (long) length, error));
            check((int) htmlPushUtf16.invokeExact(encoded, MemorySegment.ofArray(units), (long) length, error));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void htmlFree(long html) {
        try {
            htmlFree.invokeExact(MemorySegment.ofAddress(html));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long parseHtml(long html, long[] queries) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment list = arena.allocateFrom(JAVA_LONG, queries);
            check((int) parseHtml.invokeExact(MemorySegment.ofAddress(html), list, (long) queries.length, s.out, s.error));
            return s.out.get(ADDRESS, 0).address();
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long parse(byte[] html, long[] queries) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment bytes = arena.allocate(html.length);
            MemorySegment.copy(html, 0, bytes, JAVA_BYTE, 0, html.length);
            MemorySegment view = arena.allocate(STRING_VIEW);
            view.set(ADDRESS, 0, bytes);
            view.set(JAVA_LONG, 8, html.length);
            MemorySegment list = arena.allocateFrom(JAVA_LONG, queries);
            check((int) parse.invokeExact(view, list, (long) queries.length, s.out, s.error));
            return s.out.get(ADDRESS, 0).address();
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public void storeFree(long store) {
        try {
            storeFree.invokeExact(MemorySegment.ofAddress(store));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long storeLength(long store) {
        try {
            return (long) storeLen.invokeExact(MemorySegment.ofAddress(store));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long[] storeGet(long store, String selector) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment view = s.view(arena, selector);
            check((int) storeGet.invokeExact(MemorySegment.ofAddress(store), view, s.out, s.error));
            return takeList(s.out.get(ADDRESS, 0));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long[] storeQuery(long store, long index) {
        Scratch s = scratch.get();
        try {
            check((int) storeQuery.invokeExact(MemorySegment.ofAddress(store), index, s.out, s.error));
            return takeList(s.out.get(ADDRESS, 0));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public long[] elementGet(long store, long element, String selector) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment view = s.view(arena, selector);
            check((int) elementGet.invokeExact(MemorySegment.ofAddress(store), element, view, s.out, s.error));
            return takeList(s.out.get(ADDRESS, 0));
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public String name(long store, long element) {
        Scratch s = scratch.get();
        try {
            check((int) elementName.invokeExact(MemorySegment.ofAddress(store), element, s.out, s.error));
            return decode(s.out, 0);
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public String field(long store, long element, Field field) {
        Scratch s = scratch.get();
        try {
            MethodHandle getter = fields[field.ordinal()];
            check((int) getter.invokeExact(MemorySegment.ofAddress(store), element, s.out, s.error));
            return decodeOptional(s.out, 0);
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public String attribute(long store, long element, String key) {
        Scratch s = scratch.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment view = s.view(arena, key);
            check((int) elementAttribute.invokeExact(MemorySegment.ofAddress(store), element, view, s.out, s.error));
            return decodeOptional(s.out, 0);
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    @Override
    public String[] attributes(long store, long element) {
        Scratch s = scratch.get();
        try {
            MemorySegment address = MemorySegment.ofAddress(store);
            check((int) attributeCount.invokeExact(address, element, s.out, s.error));
            long count = s.out.get(JAVA_LONG, 0);
            String[] pairs = new String[Math.toIntExact(count * 2)];
            MemorySegment key = s.out.asSlice(Scratch.KEY_OFFSET, STRING_VIEW);
            for (int i = 0; i < count; i++) {
                check((int) attributeAt.invokeExact(address, element, (long) i, key, s.out, s.error));
                pairs[2 * i] = decode(key, 0);
                pairs[2 * i + 1] = decodeOptional(s.out, 0);
            }
            return pairs;
        } catch (Throwable e) {
            throw rethrow(e);
        }
    }

    /** Copy a native element list's ids and free it. NULL is an empty list. */
    private long[] takeList(MemorySegment list) throws Throwable {
        if (list.address() == 0) {
            return new long[0];
        }
        try {
            long length = (long) listLen.invokeExact(list);
            MemorySegment ids = (MemorySegment) listIds.invokeExact(list);
            return ids.reinterpret(length * JAVA_LONG.byteSize()).toArray(JAVA_LONG);
        } finally {
            listFree.invokeExact(list);
        }
    }

    /** Decode the {@code ScahStringView} at {@code offset}. */
    private static String decode(MemorySegment segment, long offset) {
        long length = segment.get(JAVA_LONG, offset + 8);
        if (length == 0) {
            return "";
        }
        byte[] bytes = segment.get(ADDRESS, offset).reinterpret(length).toArray(JAVA_BYTE);
        return new String(bytes, StandardCharsets.UTF_8);
    }

    /** Decode the {@code ScahOptionalStringView} at {@code offset}, or null when it is missing. */
    private static String decodeOptional(MemorySegment segment, long offset) {
        return segment.get(JAVA_BOOLEAN, offset) ? decode(segment, offset + 8) : null;
    }

    /** Throw the error a failed call left in the scratch area, freeing it. */
    private void check(int status) throws Throwable {
        if (status == 0) {
            return;
        }
        MemorySegment error = scratch.get().error.get(ADDRESS, 0);
        String message;
        try (Arena arena = Arena.ofConfined()) {
            message = error.address() == 0
                    ? "scah call failed with status " + status
                    : decode((MemorySegment) errorMessage.invokeExact((SegmentAllocator) arena, error), 0);
        } finally {
            errorFree.invokeExact(error);
        }
        throw new ScahException(status, message);
    }

    private static RuntimeException rethrow(Throwable e) {
        if (e instanceof RuntimeException runtime) {
            throw runtime;
        }
        if (e instanceof Error error) {
            throw error;
        }
        throw new IllegalStateException(e);
    }

    /** Per-thread native memory for out-parameters and short input strings. */
    private static final class Scratch {
        /** Strings up to this many UTF-8 bytes are copied into the scratch area. */
        static final int INLINE_STRING = 256;

        static final long KEY_OFFSET = 32;

        // Freed by the garbage collector once its thread's scratch is unreachable.
        private final Arena arena = Arena.ofAuto();
        /** A pointer, a count, or a string view at 0; a second string view at KEY_OFFSET. */
        final MemorySegment out = arena.allocate(64, 8);
        final MemorySegment error = arena.allocate(ADDRESS);
        private final MemorySegment view = arena.allocate(STRING_VIEW);
        private final MemorySegment save = arena.allocate(SAVE);
        private final MemorySegment text = arena.allocate(INLINE_STRING);

        /** A {@code ScahStringView} of {@code value}, in the scratch area or in {@code arena}. */
        MemorySegment view(Arena overflow, String value) {
            byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
            MemorySegment data = bytes.length <= INLINE_STRING ? text : overflow.allocate(bytes.length);
            MemorySegment.copy(bytes, 0, data, JAVA_BYTE, 0, bytes.length);
            view.set(ADDRESS, 0, data);
            view.set(JAVA_LONG, 8, bytes.length);
            return view;
        }

        MemorySegment save(Save value) {
            save.set(JAVA_BOOLEAN, 0, value.innerHtml());
            save.set(JAVA_BOOLEAN, 1, value.rawText());
            save.set(JAVA_BOOLEAN, 2, value.text());
            save.set(JAVA_BOOLEAN, 3, value.attributes());
            return save;
        }
    }

    /** Looks up functions in the library. */
    private static final class Binder {
        private final SymbolLookup symbols;
        private final Linker linker = Linker.nativeLinker();

        Binder(SymbolLookup symbols) {
            this.symbols = symbols;
        }

        MethodHandle function(String name, FunctionDescriptor descriptor) {
            return linker.downcallHandle(find(name), descriptor);
        }

        /** A short, bounded call that only reads, linked without a thread-state transition. */
        MethodHandle reader(String name, FunctionDescriptor descriptor) {
            return linker.downcallHandle(find(name), descriptor, Linker.Option.critical(false));
        }

        /**
         * A short, bounded call that may also read Java arrays in place, linked without a
         * thread-state transition.
         */
        MethodHandle arrayReader(String name, FunctionDescriptor descriptor) {
            return linker.downcallHandle(find(name), descriptor, Linker.Option.critical(true));
        }

        private MemorySegment find(String name) {
            return symbols.find(name).orElseThrow(() -> new UnsatisfiedLinkError("scah_c has no symbol " + name));
        }
    }
}
