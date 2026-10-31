using System.Collections;

namespace Scah;

/// <summary>The elements one selector matched, in document order.</summary>
public sealed class ElementList : IReadOnlyList<Element>
{
    private readonly Store store;
    private readonly nuint[] ids;

    private ElementList(Store store, nuint[] ids)
    {
        this.store = store;
        this.ids = ids;
    }

    /// <summary>Copy a native list's ids and free it. NULL is an empty list.</summary>
    internal static unsafe ElementList Take(Store store, nint list)
    {
        if (list == nint.Zero)
        {
            return new(store, []);
        }
        try
        {
            int count = checked((int)Native.scah_element_list_len(list));
            var ids = new ReadOnlySpan<nuint>(Native.scah_element_list_ids(list), count).ToArray();
            return new(store, ids);
        }
        finally
        {
            Native.scah_element_list_free(list);
        }
    }

    /// <inheritdoc/>
    public int Count => ids.Length;

    /// <inheritdoc/>
    public Element this[int index] => new(store, ids[index]);

    /// <summary>Enumerate the elements without allocating.</summary>
    public Enumerator GetEnumerator() => new(this);

    IEnumerator<Element> IEnumerable<Element>.GetEnumerator() => GetEnumerator();

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    /// <summary>Enumerates an <see cref="ElementList"/>.</summary>
    public struct Enumerator : IEnumerator<Element>
    {
        private readonly ElementList list;
        private int index;

        internal Enumerator(ElementList list)
        {
            this.list = list;
            index = -1;
        }

        /// <inheritdoc/>
        public readonly Element Current => list[index];

        readonly object IEnumerator.Current => Current;

        /// <inheritdoc/>
        public bool MoveNext() => ++index < list.Count;

        /// <inheritdoc/>
        public void Reset() => index = -1;

        /// <inheritdoc/>
        public readonly void Dispose() { }
    }
}
