using AngleSharp.Html.Parser;
using Fizzler.Systems.HtmlAgilityPack;
using HtmlAgilityPack;

namespace Scah.Benchmarks;

/// <summary>
/// One way per library to parse a document and read what it selects.
/// </summary>
/// <remarks>
/// Every scraper reads the inner HTML, the text, and the <c>href</c> of each element it finds,
/// and returns how many it found. The nested scenario reads every product card and its title,
/// rating, and description.
/// </remarks>
internal abstract class Scraper
{
    public abstract int All(string html, string selector, Sink sink);

    public abstract int First(string html, string selector, Sink sink);

    public abstract int Nested(string html, Sink sink);

    public static Scraper Named(string name) => name switch
    {
        "scah" => new ScahScraper(),
        "AngleSharp" => new AngleSharpScraper(),
        "HtmlAgilityPack" => new HtmlAgilityPackScraper(),
        "HtmlAgilityPack + Fizzler" => new FizzlerScraper(),
        _ => throw new ArgumentException($"unknown library {name}", nameof(name)),
    };
}

internal sealed class ScahScraper : Scraper
{
    private static readonly Save Save = Save.OnlyInnerHtml with { Text = true };

    public override int All(string html, string selector, Sink sink)
    {
        using var query = Query.All(selector, Save).Build();
        using var store = Store.Parse(html, query);
        var elements = store.Get(selector);
        foreach (var element in elements)
        {
            Read(element, sink);
        }
        return elements.Count;
    }

    public override int First(string html, string selector, Sink sink)
    {
        using var query = Query.First(selector, Save).Build();
        using var store = Store.Parse(html, query);
        var elements = store.Get(selector);
        if (elements.Count == 0)
        {
            return 0;
        }
        Read(elements[0], sink);
        return 1;
    }

    public override int Nested(string html, Sink sink)
    {
        using var query = Query.All(".product", Save)
            .Then(
                Query.First("> h1", Save),
                Query.First("> .rating", Save),
                Query.First("> .description", Save))
            .Build();
        using var store = Store.Parse(html, query);
        var products = store.Get(".product");
        foreach (var product in products)
        {
            Read(product, sink);
            Read(product.Get("> h1")[0], sink);
            Read(product.Get("> .rating")[0], sink);
            Read(product.Get("> .description")[0], sink);
        }
        return products.Count;
    }

    private static void Read(Element element, Sink sink)
    {
        sink.Consume(element.InnerHtml);
        sink.Consume(element.Text);
        sink.Consume(element.GetAttribute("href"));
    }
}

internal sealed class AngleSharpScraper : Scraper
{
    private readonly HtmlParser parser = new();

    public override int All(string html, string selector, Sink sink)
    {
        using var document = parser.ParseDocument(html);
        var elements = document.QuerySelectorAll(selector);
        foreach (var element in elements)
        {
            Read(element, sink);
        }
        return elements.Length;
    }

    public override int First(string html, string selector, Sink sink)
    {
        using var document = parser.ParseDocument(html);
        var element = document.QuerySelector(selector);
        if (element is null)
        {
            return 0;
        }
        Read(element, sink);
        return 1;
    }

    public override int Nested(string html, Sink sink)
    {
        using var document = parser.ParseDocument(html);
        var products = document.QuerySelectorAll(".product");
        foreach (var product in products)
        {
            Read(product, sink);
            Read(product.QuerySelector("h1")!, sink);
            Read(product.QuerySelector(".rating")!, sink);
            Read(product.QuerySelector(".description")!, sink);
        }
        return products.Length;
    }

    private static void Read(AngleSharp.Dom.IElement element, Sink sink)
    {
        sink.Consume(element.InnerHtml);
        sink.Consume(element.TextContent);
        sink.Consume(element.GetAttribute("href"));
    }
}

/// <summary>HtmlAgilityPack's own query language, XPath.</summary>
internal sealed class HtmlAgilityPackScraper : Scraper
{
    private static string HasClass(string name) =>
        $"contains(concat(' ', normalize-space(@class), ' '), ' {name} ')";

    private static readonly string Products = $"//*[{HasClass("product")}]";
    private static readonly string Rating = $".//*[{HasClass("rating")}]";
    private static readonly string Description = $".//*[{HasClass("description")}]";

    public override int All(string html, string tag, Sink sink)
    {
        var elements = Load(html).DocumentNode.SelectNodes($"//{tag}");
        if (elements is null)
        {
            return 0;
        }
        foreach (var element in elements)
        {
            HtmlAgilityPackReader.Read(element, sink);
        }
        return elements.Count;
    }

    public override int First(string html, string tag, Sink sink)
    {
        var element = Load(html).DocumentNode.SelectSingleNode($"//{tag}");
        if (element is null)
        {
            return 0;
        }
        HtmlAgilityPackReader.Read(element, sink);
        return 1;
    }

    public override int Nested(string html, Sink sink)
    {
        var products = Load(html).DocumentNode.SelectNodes(Products);
        if (products is null)
        {
            return 0;
        }
        foreach (var product in products)
        {
            HtmlAgilityPackReader.Read(product, sink);
            HtmlAgilityPackReader.Read(product.SelectSingleNode(".//h1")!, sink);
            HtmlAgilityPackReader.Read(product.SelectSingleNode(Rating)!, sink);
            HtmlAgilityPackReader.Read(product.SelectSingleNode(Description)!, sink);
        }
        return products.Count;
    }

    internal static HtmlDocument Load(string html)
    {
        var document = new HtmlDocument();
        document.LoadHtml(html);
        return document;
    }
}

/// <summary>HtmlAgilityPack with CSS selectors from Fizzler.</summary>
internal sealed class FizzlerScraper : Scraper
{
    public override int All(string html, string selector, Sink sink)
    {
        var count = 0;
        foreach (var element in HtmlAgilityPackScraper.Load(html).DocumentNode.QuerySelectorAll(selector))
        {
            HtmlAgilityPackReader.Read(element, sink);
            count++;
        }
        return count;
    }

    public override int First(string html, string selector, Sink sink)
    {
        var element = HtmlAgilityPackScraper.Load(html).DocumentNode.QuerySelector(selector);
        if (element is null)
        {
            return 0;
        }
        HtmlAgilityPackReader.Read(element, sink);
        return 1;
    }

    public override int Nested(string html, Sink sink)
    {
        var count = 0;
        foreach (var product in HtmlAgilityPackScraper.Load(html).DocumentNode.QuerySelectorAll(".product"))
        {
            HtmlAgilityPackReader.Read(product, sink);
            HtmlAgilityPackReader.Read(product.QuerySelector("h1")!, sink);
            HtmlAgilityPackReader.Read(product.QuerySelector(".rating")!, sink);
            HtmlAgilityPackReader.Read(product.QuerySelector(".description")!, sink);
            count++;
        }
        return count;
    }
}

internal static class HtmlAgilityPackReader
{
    public static void Read(HtmlNode element, Sink sink)
    {
        sink.Consume(element.InnerHtml);
        sink.Consume(element.InnerText);
        sink.Consume(element.GetAttributeValue("href", null));
    }
}

/// <summary>Keeps each value it is given alive, so the JIT cannot skip computing it.</summary>
internal sealed class Sink
{
    private string? last;

    public void Consume(string? value) => Volatile.Write(ref last, value);
}
