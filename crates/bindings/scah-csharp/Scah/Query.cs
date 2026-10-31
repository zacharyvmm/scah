namespace Scah;

/// <summary>A compiled, immutable query, ready to pass to <see cref="Store.Parse(string, Query[])"/>.</summary>
/// <remarks>
/// Queries may be shared across threads and reused for any number of parses.
/// A store keeps what it needs of its queries, so a query may be disposed as
/// soon as parsing returns. Undisposed queries are freed by the finalizer.
/// </remarks>
public sealed class Query : IDisposable
{
    internal Query(QueryHandle handle) => Handle = handle;

    internal QueryHandle Handle { get; }

    /// <summary>
    /// Start a query whose first section matches every element selected by
    /// <paramref name="selector"/>.
    /// </summary>
    public static QueryBuilder All(string selector, Save save) => QueryBuilder.Start(first: false, selector, save);

    /// <summary>
    /// Start a query whose first section matches only the first element
    /// selected by <paramref name="selector"/>, which lets parsing stop early.
    /// </summary>
    public static QueryBuilder First(string selector, Save save) => QueryBuilder.Start(first: true, selector, save);

    /// <summary>Free the compiled query. Stores parsed with it stay valid.</summary>
    public void Dispose() => Handle.Dispose();
}
