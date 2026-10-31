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
/// Every read takes a <see cref="StoreLease"/>, which keeps the store alive
/// even if another thread disposes it meanwhile.
/// </remarks>
internal sealed class StoreHandle : SafeHandle
{
    private volatile bool disposed;

    public StoreHandle()
        : base(nint.Zero, ownsHandle: true) { }

    internal StoreHandle(nint handle)
        : this() => SetHandle(handle);

    public override bool IsInvalid => handle == nint.Zero;

    /// <summary>
    /// Whether <see cref="SafeHandle.Dispose()"/> was called. Unlike
    /// <see cref="SafeHandle.IsClosed"/>, this is set even while reads still
    /// hold the handle.
    /// </summary>
    internal bool IsDisposed => disposed;

    protected override void Dispose(bool disposing)
    {
        disposed = true;
        base.Dispose(disposing);
    }

    protected override bool ReleaseHandle()
    {
        Native.scah_store_free(handle);
        return true;
    }
}

/// <summary>
/// Keeps a store alive from before a native call until the string views it
/// returned are decoded, which a <see cref="SafeHandle"/> argument alone would
/// not: the marshaller releases it as soon as the call returns.
/// </summary>
internal readonly ref struct StoreLease
{
    private readonly StoreHandle handle;

    /// <summary>The store's address, valid until <see cref="Dispose"/>.</summary>
    public readonly nint Pointer;

    /// <exception cref="ObjectDisposedException">The store is disposed.</exception>
    public StoreLease(StoreHandle handle)
    {
        bool added = false;
        handle.DangerousAddRef(ref added);
        // A SafeHandle accepts new references until the last one is released,
        // so overlapping reads would otherwise keep a disposed store readable.
        if (handle.IsDisposed)
        {
            handle.DangerousRelease();
            throw new ObjectDisposedException(nameof(Store));
        }
        this.handle = handle;
        Pointer = handle.DangerousGetHandle();
    }

    public void Dispose() => handle.DangerousRelease();
}
