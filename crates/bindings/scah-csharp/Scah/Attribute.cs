namespace Scah;

/// <summary>An attribute of an element, as written in the source.</summary>
/// <param name="Name">The attribute's name.</param>
/// <param name="Value">
/// The attribute's value, or <see langword="null"/> for an attribute without
/// one, as in <c>&lt;input disabled&gt;</c>.
/// </param>
public readonly record struct Attribute(string Name, string? Value);
