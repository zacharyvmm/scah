namespace Scah;

/// <summary>An element a query matched: a row of its <see cref="Store"/>.</summary>
/// <remarks>
/// Elements are cheap handles; each property reads the store and returns a
/// new string. An element keeps its store from being finalized, but not from
/// being disposed: reading an element of a disposed store throws
/// <see cref="ObjectDisposedException"/>. Properties whose content the query
/// did not save are <see langword="null"/>.
/// </remarks>
public readonly struct Element : IEquatable<Element>
{
    private readonly Store store;
    private readonly nuint id;

    internal Element(Store store, nuint id)
    {
        this.store = store;
        this.id = id;
    }

    /// <summary>The store this element belongs to.</summary>
    public Store Store => store ?? throw Uninitialized();

    /// <summary>The element's row in its store, unique within it.</summary>
    public long Index => (long)id;

    /// <summary>The tag name.</summary>
    public unsafe string Name
    {
        get
        {
            StringView value;
            nint error;
            Native.Check(Native.scah_element_name(Handle, id, &value, &error), error);
            return value.Decode();
        }
    }

    /// <summary>The <c>id</c> attribute, if attributes were saved.</summary>
    public unsafe string? Id => Read(&Native.scah_element_id);

    /// <summary>The <c>class</c> attribute, if attributes were saved.</summary>
    public unsafe string? ClassName => Read(&Native.scah_element_class_name);

    /// <summary>The HTML between the element's tags, if saved.</summary>
    public unsafe string? InnerHtml => Read(&Native.scah_element_inner_html);

    /// <summary>The descendant text as written in the source, if saved.</summary>
    public unsafe string? RawText => Read(&Native.scah_element_raw_text);

    /// <summary>
    /// The descendant text with entities decoded and whitespace collapsed, if saved.
    /// </summary>
    public unsafe string? Text => Read(&Native.scah_element_text);

    /// <summary>
    /// The attributes other than <c>id</c> and <c>class</c>, in source order,
    /// or an empty list when attributes were not saved.
    /// </summary>
    public unsafe IReadOnlyList<Attribute> Attributes
    {
        get
        {
            StoreHandle handle = Handle;
            nuint count;
            nint error;
            Native.Check(Native.scah_element_attribute_count(handle, id, &count, &error), error);
            if (count == 0)
            {
                return [];
            }
            var attributes = new Attribute[checked((int)count)];
            for (int i = 0; i < attributes.Length; i++)
            {
                StringView key;
                OptionalStringView value;
                Native.Check(
                    Native.scah_element_attribute_at(handle, id, (nuint)i, &key, &value, &error),
                    error);
                attributes[i] = new Attribute(key.Decode(), value.Decode());
            }
            return attributes;
        }
    }

    /// <summary>
    /// The value of the attribute named <paramref name="name"/>, compared ASCII
    /// case-insensitively.
    /// </summary>
    /// <returns>
    /// The value, or <see langword="null"/> when the attribute is absent, has no
    /// value (as in <c>&lt;input disabled&gt;</c>), or attributes were not saved.
    /// </returns>
    public unsafe string? GetAttribute(string name)
    {
        using Utf8.Buffer bytes = Utf8.Encode(name, stackalloc byte[Utf8.StackLimit]);
        fixed (byte* data = bytes.Bytes)
        {
            OptionalStringView value;
            nint error;
            int status = Native.scah_element_attribute(
                Handle, id, new StringView(data, (nuint)bytes.Bytes.Length), &value, &error);
            Native.Check(status, error);
            return value.Decode();
        }
    }

    /// <summary>
    /// The elements matched under this one by the section built with
    /// <paramref name="selector"/>: the section chained after this element's
    /// with <see cref="QueryBuilder.All"/> or <see cref="QueryBuilder.First"/>,
    /// or a child query nested with <see cref="QueryBuilder.Then(QueryBuilder[])"/>.
    /// </summary>
    /// <remarks>
    /// The list is empty when nothing matched or no such section has that selector.
    /// </remarks>
    public unsafe ElementList Get(string selector)
    {
        using Utf8.Buffer bytes = Utf8.Encode(selector, stackalloc byte[Utf8.StackLimit]);
        fixed (byte* data = bytes.Bytes)
        {
            nint list;
            nint error;
            int status = Native.scah_element_get(
                Handle, id, new StringView(data, (nuint)bytes.Bytes.Length), &list, &error);
            Native.Check(status, error);
            return ElementList.Take(store, list);
        }
    }

    /// <inheritdoc/>
    public bool Equals(Element other) => ReferenceEquals(store, other.store) && id == other.id;

    /// <inheritdoc/>
    public override bool Equals(object? obj) => obj is Element other && Equals(other);

    /// <inheritdoc/>
    public override int GetHashCode() => HashCode.Combine(store, id);

    /// <summary>Whether two elements are the same row of the same store.</summary>
    public static bool operator ==(Element left, Element right) => left.Equals(right);

    /// <summary>Whether two elements differ.</summary>
    public static bool operator !=(Element left, Element right) => !left.Equals(right);

    private StoreHandle Handle => Store.Handle;

    private unsafe string? Read(
        delegate*<StoreHandle, nuint, OptionalStringView*, nint*, int> getter)
    {
        OptionalStringView value;
        nint error;
        Native.Check(getter(Handle, id, &value, &error), error);
        return value.Decode();
    }

    private static InvalidOperationException Uninitialized() =>
        new("This element is default-initialized and belongs to no store.");
}
