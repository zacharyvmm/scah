using System.Runtime.InteropServices;

namespace Scah;

/// <summary>Owns a compiled <c>ScahQuery</c>.</summary>
internal sealed class QueryHandle : SafeHandle
{
    public QueryHandle()
        : base(nint.Zero, ownsHandle: true) { }

    internal QueryHandle(nint handle)
        : this() => SetHandle(handle);

    public override bool IsInvalid => handle == nint.Zero;

    protected override bool ReleaseHandle()
    {
        Native.scah_query_free(handle);
        return true;
    }
}

/// <summary>Owns a parsed <c>ScahStore</c>.</summary>
/// <remarks>
/// Every call that reads the store passes this handle, which keeps the store
/// alive for the duration of the call even if another thread disposes it.
/// </remarks>
internal sealed class StoreHandle : SafeHandle
{
    public StoreHandle()
        : base(nint.Zero, ownsHandle: true) { }

    internal StoreHandle(nint handle)
        : this() => SetHandle(handle);

    public override bool IsInvalid => handle == nint.Zero;

    protected override bool ReleaseHandle()
    {
        Native.scah_store_free(handle);
        return true;
    }
}
