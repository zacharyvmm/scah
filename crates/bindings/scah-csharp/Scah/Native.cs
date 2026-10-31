using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

// Every native signature uses blittable types only, so calls skip the
// runtime's marshalling layer.
[assembly: DisableRuntimeMarshalling]

namespace Scah;

/// <summary>A UTF-8 string borrowed across the C ABI: <c>ScahStringView</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
internal unsafe readonly struct StringView(byte* data, nuint length)
{
    public readonly byte* Data = data;
    public readonly nuint Length = length;

    public string Decode() => Utf8.Decode(Data, Length);
}

/// <summary>A string view that may be absent: <c>ScahOptionalStringView</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct OptionalStringView
{
    // C `bool`, one byte.
    public readonly byte IsSome;
    public readonly StringView Value;

    public string? Decode() => IsSome != 0 ? Value.Decode() : null;
}

/// <summary><c>ScahSave</c>: one C <c>bool</c> per field.</summary>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeSave(Save save)
{
    public readonly byte InnerHtml = save.InnerHtml ? (byte)1 : (byte)0;
    public readonly byte RawText = save.RawText ? (byte)1 : (byte)0;
    public readonly byte Text = save.Text ? (byte)1 : (byte)0;
    public readonly byte Attributes = save.Attributes ? (byte)1 : (byte)0;
}

/// <summary>The C ABI declared by <c>crates/bindings/scah-c/include/scah.h</c>.</summary>
/// <remarks>
/// Statuses are returned as <see cref="int"/> so that codes added by a newer
/// library still map to an exception. Builders, element lists, and errors are
/// passed as raw pointers because they never outlive the call that uses them;
/// queries and stores are <see cref="SafeHandle"/>s.
/// </remarks>
internal static unsafe partial class Native
{
    private const string Library = "scah_c";

    /// <summary>The C ABI version these declarations match.</summary>
    internal const uint AbiVersion = 1;

    static Native()
    {
        uint loaded = scah_abi_version();
        if (loaded != AbiVersion)
        {
            throw new InvalidOperationException(
                $"The loaded {Library} library has ABI version {loaded}, but this "
                + $"assembly needs version {AbiVersion}.");
        }
    }

    /// <summary>Run the ABI check, if it has not run yet.</summary>
    internal static void EnsureLoaded() { }

    [LibraryImport(Library)]
    internal static partial uint scah_abi_version();

    [LibraryImport(Library)]
    internal static partial StringView scah_error_message(nint error);

    [LibraryImport(Library)]
    internal static partial void scah_error_free(nint error);

    [LibraryImport(Library)]
    internal static partial int scah_query_builder_new_all(
        StringView selector, NativeSave save, nint* builder, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_query_builder_new_first(
        StringView selector, NativeSave save, nint* builder, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_query_builder_all(
        nint builder, StringView selector, NativeSave save, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_query_builder_first(
        nint builder, StringView selector, NativeSave save, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_query_builder_then(
        nint builder, nint* children, nuint count, nint* error);

    [LibraryImport(Library)]
    internal static partial void scah_query_builder_free(nint builder);

    [LibraryImport(Library)]
    internal static partial int scah_query_build(nint builder, nint* query, nint* error);

    [LibraryImport(Library)]
    internal static partial void scah_query_free(nint query);

    [LibraryImport(Library)]
    internal static partial int scah_parse(
        StringView html, nint* queries, nuint count, nint* store, nint* error);

    [LibraryImport(Library)]
    internal static partial void scah_store_free(nint store);

    [LibraryImport(Library)]
    internal static partial nuint scah_store_len(StoreHandle store);

    [LibraryImport(Library)]
    internal static partial int scah_store_get(
        StoreHandle store, StringView selector, nint* list, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_store_query(
        StoreHandle store, nuint index, nint* list, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_get(
        StoreHandle store, nuint element, StringView selector, nint* list, nint* error);

    [LibraryImport(Library)]
    internal static partial nuint scah_element_list_len(nint list);

    [LibraryImport(Library)]
    internal static partial nuint* scah_element_list_ids(nint list);

    [LibraryImport(Library)]
    internal static partial void scah_element_list_free(nint list);

    [LibraryImport(Library)]
    internal static partial int scah_element_name(
        StoreHandle store, nuint element, StringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_id(
        StoreHandle store, nuint element, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_class_name(
        StoreHandle store, nuint element, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_inner_html(
        StoreHandle store, nuint element, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_raw_text(
        StoreHandle store, nuint element, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_text(
        StoreHandle store, nuint element, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_attribute(
        StoreHandle store, nuint element, StringView key, OptionalStringView* value, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_attribute_count(
        StoreHandle store, nuint element, nuint* count, nint* error);

    [LibraryImport(Library)]
    internal static partial int scah_element_attribute_at(
        StoreHandle store, nuint element, nuint index, StringView* key,
        OptionalStringView* value, nint* error);

    /// <summary>Throw the error a failed call reported, freeing it.</summary>
    internal static void Check(int status, nint error)
    {
        if (status != 0)
        {
            throw ScahException.FromNative(status, error);
        }
    }
}
