namespace Scah;

/// <summary>What to keep for each element a query section matches.</summary>
/// <remarks>
/// The tag name is always kept. <c>default(Save)</c> keeps nothing else; see
/// <see cref="NameOnly"/>.
/// </remarks>
public readonly record struct Save
{
    /// <summary>Keep the HTML between the opening and closing tags.</summary>
    public bool InnerHtml { get; init; }

    /// <summary>Keep the descendant text exactly as written in the source.</summary>
    public bool RawText { get; init; }

    /// <summary>
    /// Keep the descendant text with entities decoded and whitespace collapsed.
    /// </summary>
    public bool Text { get; init; }

    /// <summary>Keep the <c>id</c>, <c>class</c>, and other attributes.</summary>
    public bool Attributes { get; init; }

    /// <summary>Keep everything: inner HTML, raw text, text, and attributes.</summary>
    public static Save All => new() { InnerHtml = true, RawText = true, Text = true, Attributes = true };

    /// <summary>Keep attributes, but neither inner HTML nor text.</summary>
    public static Save None => new() { Attributes = true };

    /// <summary>Keep only the tag name.</summary>
    public static Save NameOnly => default;

    /// <summary>Keep the inner HTML and attributes.</summary>
    public static Save OnlyInnerHtml => new() { InnerHtml = true, Attributes = true };

    /// <summary>Keep the raw text and attributes.</summary>
    public static Save OnlyRawText => new() { RawText = true, Attributes = true };

    /// <summary>Keep the text and attributes.</summary>
    public static Save OnlyText => new() { Text = true, Attributes = true };
}
