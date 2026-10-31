namespace Scah;

/// <summary>The elements that one or more queries matched in a document.</summary>
/// <remarks>
/// A store owns a copy of the HTML and is immutable, so it may be read from
/// several threads at once. Dispose it to free its memory deterministically;
/// reading a disposed store throws <see cref="ObjectDisposedException"/>.
/// Undisposed stores are freed by the finalizer once no element refers to them.
/// </remarks>
public sealed class Store : IDisposable
{
    private Store(StoreHandle handle) => Handle = handle;

    internal StoreHandle Handle { get; }

    /// <summary>Parse <paramref name="html"/> against <paramref name="queries"/>.</summary>
    /// <exception cref="ScahException">
    /// No queries were given, or the HTML nests too deeply or is 4 GiB or longer.
    /// </exception>
    public static Store Parse(string html, params Query[] queries)
    {
        ArgumentNullException.ThrowIfNull(html);
        using Utf8.Buffer bytes = Utf8.Encode(html, stackalloc byte[Utf8.StackLimit]);
        return Parse(bytes.Bytes, queries);
    }

    /// <summary>Parse UTF-8 <paramref name="html"/> against <paramref name="queries"/>.</summary>
    /// <exception cref="ScahException">
    /// The HTML is not valid UTF-8, no queries were given, or the HTML nests too
    /// deeply or is 4 GiB or longer.
    /// </exception>
    public static unsafe Store Parse(ReadOnlySpan<byte> html, params Query[] queries)
    {
        ArgumentNullException.ThrowIfNull(queries);
        Native.EnsureLoaded();
        var handles = new nint[queries.Length];
        int referenced = 0;
        try
        {
            // Keep every query alive while the native call reads it.
            for (; referenced < queries.Length; referenced++)
            {
                ArgumentNullException.ThrowIfNull(queries[referenced], nameof(queries));
                bool added = false;
                queries[referenced].Handle.DangerousAddRef(ref added);
                handles[referenced] = queries[referenced].Handle.DangerousGetHandle();
            }
            fixed (byte* data = html)
            fixed (nint* list = handles)
            {
                nint store;
                nint error;
                int status = Native.scah_parse(
                    new StringView(data, (nuint)html.Length), list, (nuint)handles.Length, &store, &error);
                Native.Check(status, error);
                return new Store(new StoreHandle(store));
            }
        }
        finally
        {
            for (int i = 0; i < referenced; i++)
            {
                queries[i].Handle.DangerousRelease();
            }
        }
    }

    /// <summary>The number of elements saved, across all queries.</summary>
    public long ElementCount
    {
        get
        {
            using var store = new StoreLease(Handle);
            return (long)Native.scah_store_len(store.Pointer);
        }
    }

    /// <summary>
    /// The elements matched by the top-level section built with
    /// <paramref name="selector"/>, in document order.
    /// </summary>
    /// <remarks>
    /// <paramref name="selector"/> must be the exact string the section was
    /// built with. When several queries share it, the first one's results are
    /// returned; use <see cref="QueryResults"/> to pick another. The list is
    /// empty when nothing matched or no section has that selector.
    /// </remarks>
    public unsafe ElementList Get(string selector)
    {
        using Utf8.Buffer bytes = Utf8.Encode(selector, stackalloc byte[Utf8.StackLimit]);
        using var store = new StoreLease(Handle);
        fixed (byte* data = bytes.Bytes)
        {
            nint list;
            nint error;
            int status = Native.scah_store_get(
                store.Pointer, new StringView(data, (nuint)bytes.Bytes.Length), &list, &error);
            Native.Check(status, error);
            return ElementList.Take(this, list);
        }
    }

    /// <summary>
    /// The elements matched by the query at <paramref name="index"/> in the
    /// array given to <see cref="Parse(string, Query[])"/>, in document order.
    /// </summary>
    /// <remarks>The list is empty when nothing matched or there is no such query.</remarks>
    public unsafe ElementList QueryResults(int index)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(index);
        using var store = new StoreLease(Handle);
        nint list;
        nint error;
        Native.Check(Native.scah_store_query(store.Pointer, (nuint)index, &list, &error), error);
        return ElementList.Take(this, list);
    }

    /// <summary>Free the store. Elements read from it can no longer be read.</summary>
    public void Dispose() => Handle.Dispose();
}
