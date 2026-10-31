namespace Scah;

/// <summary>
/// Describes a query: a chain of selector sections, each of which may have
/// nested child queries. <see cref="Build"/> compiles it into a <see cref="Query"/>.
/// </summary>
/// <remarks>
/// Builders are immutable: <see cref="All"/>, <see cref="First"/>, and
/// <see cref="Then(QueryBuilder[])"/> return a new builder, so a shared prefix
/// can be extended in several ways. They hold no native resources.
/// </remarks>
public sealed class QueryBuilder
{
    private readonly Step[] steps;

    private QueryBuilder(Step[] steps) => this.steps = steps;

    internal static QueryBuilder Start(bool first, string selector, Save save)
    {
        ArgumentNullException.ThrowIfNull(selector);
        return new([Step.Section(first, selector, save)]);
    }

    /// <summary>
    /// Append a section that matches every element selected by
    /// <paramref name="selector"/> inside each match of the last section.
    /// </summary>
    public QueryBuilder All(string selector, Save save)
    {
        ArgumentNullException.ThrowIfNull(selector);
        return Append(Step.Section(first: false, selector, save));
    }

    /// <summary>
    /// Append a section that matches only the first element selected by
    /// <paramref name="selector"/> inside each match of the last section.
    /// </summary>
    public QueryBuilder First(string selector, Save save)
    {
        ArgumentNullException.ThrowIfNull(selector);
        return Append(Step.Section(first: true, selector, save));
    }

    /// <summary>
    /// Nest <paramref name="children"/> under the last section. Each child
    /// runs inside every match of that section; read its results with
    /// <see cref="Element.Get"/>.
    /// </summary>
    public QueryBuilder Then(params QueryBuilder[] children)
    {
        ArgumentNullException.ThrowIfNull(children);
        foreach (QueryBuilder child in children)
        {
            ArgumentNullException.ThrowIfNull(child, nameof(children));
        }
        return Append(Step.Children((QueryBuilder[])children.Clone()));
    }

    /// <inheritdoc cref="Then(QueryBuilder[])"/>
    public QueryBuilder Then(IEnumerable<QueryBuilder> children)
    {
        ArgumentNullException.ThrowIfNull(children);
        return Then(children.ToArray());
    }

    /// <summary>Compile the query.</summary>
    /// <exception cref="ScahException">
    /// A selector is invalid (<see cref="ScahStatus.InvalidSelector"/>).
    /// </exception>
    public unsafe Query Build()
    {
        nint builder = ToNative();
        try
        {
            nint query;
            nint error;
            Native.Check(Native.scah_query_build(builder, &query, &error), error);
            return new Query(new QueryHandle(query));
        }
        finally
        {
            Native.scah_query_builder_free(builder);
        }
    }

    private QueryBuilder Append(Step step)
    {
        var next = new Step[steps.Length + 1];
        steps.CopyTo(next, 0);
        next[^1] = step;
        return new(next);
    }

    /// <summary>Replay the steps into a native builder, which the caller frees.</summary>
    private unsafe nint ToNative()
    {
        Native.EnsureLoaded();
        Step head = steps[0];
        nint builder;
        nint error;
        using (Utf8.Buffer selector = Utf8.Encode(head.Selector!, stackalloc byte[Utf8.StackLimit]))
        {
            fixed (byte* data = selector.Bytes)
            {
                var view = new StringView(data, (nuint)selector.Bytes.Length);
                int status = head.First
                    ? Native.scah_query_builder_new_first(view, new NativeSave(head.Save), &builder, &error)
                    : Native.scah_query_builder_new_all(view, new NativeSave(head.Save), &builder, &error);
                Native.Check(status, error);
            }
        }

        try
        {
            for (int i = 1; i < steps.Length; i++)
            {
                if (steps[i].Nested is { } children)
                {
                    AppendChildren(builder, children);
                }
                else
                {
                    AppendSection(builder, steps[i]);
                }
            }
            return builder;
        }
        catch
        {
            Native.scah_query_builder_free(builder);
            throw;
        }
    }

    private static unsafe void AppendSection(nint builder, Step step)
    {
        using Utf8.Buffer selector = Utf8.Encode(step.Selector!, stackalloc byte[Utf8.StackLimit]);
        fixed (byte* data = selector.Bytes)
        {
            var view = new StringView(data, (nuint)selector.Bytes.Length);
            nint error;
            int status = step.First
                ? Native.scah_query_builder_first(builder, view, new NativeSave(step.Save), &error)
                : Native.scah_query_builder_all(builder, view, new NativeSave(step.Save), &error);
            Native.Check(status, error);
        }
    }

    private static unsafe void AppendChildren(nint builder, QueryBuilder[] children)
    {
        // The native builder copies its children, so they are freed right away.
        var handles = new nint[children.Length];
        try
        {
            for (int i = 0; i < children.Length; i++)
            {
                handles[i] = children[i].ToNative();
            }
            fixed (nint* data = handles)
            {
                nint error;
                Native.Check(
                    Native.scah_query_builder_then(builder, data, (nuint)handles.Length, &error),
                    error);
            }
        }
        finally
        {
            foreach (nint handle in handles)
            {
                Native.scah_query_builder_free(handle);
            }
        }
    }

    /// <summary>A section to append, or children to nest under the last section.</summary>
    private readonly record struct Step(bool First, string? Selector, Save Save, QueryBuilder[]? Nested)
    {
        public static Step Section(bool first, string selector, Save save) => new(first, selector, save, null);

        public static Step Children(QueryBuilder[] children) => new(false, null, default, children);
    }
}
