using BenchmarkDotNet.Attributes;

namespace Scah.Benchmarks;

/// <summary>
/// Parse a document and read what a query selects, for each library. The method names map to
/// the scenarios in <c>benches/results</c>: <c>SimpleAll</c> is <c>simple-all</c>, and so on.
/// </summary>
[JsonExporterAttribute.Full]
public class ScrapeBenchmark
{
    private static readonly string SimpleHtml = Inputs.Simple(Inputs.Elements);
    private static readonly string ProductHtml = Inputs.Products(Inputs.Elements);

    private readonly Sink sink = new();
    private Scraper scraper = null!;
    private string specHtml = null!;

    [Params("scah", "AngleSharp", "HtmlAgilityPack", "HtmlAgilityPack + Fizzler")]
    public string Library { get; set; } = "";

    [GlobalSetup]
    public void SetUp()
    {
        scraper = Scraper.Named(Library);
        // Fail before measuring if a library finds something other than what was asked for.
        Expect("simple-all", scraper.All(SimpleHtml, "a", sink), Inputs.Elements);
        Expect("simple-first", scraper.First(SimpleHtml, "a", sink), 1);
        Expect("nested-all", scraper.Nested(ProductHtml, sink), Inputs.Elements);
    }

    [GlobalSetup(Target = nameof(WhatwgAllLinks))]
    public void SetUpSpec()
    {
        SetUp();
        specHtml = Inputs.Spec();
    }

    private void Expect(string scenario, int found, int expected)
    {
        if (found != expected)
        {
            throw new InvalidOperationException($"{Library} found {found} elements in {scenario}, expected {expected}");
        }
    }

    [Benchmark]
    public int SimpleAll() => scraper.All(SimpleHtml, "a", sink);

    [Benchmark]
    public int SimpleFirst() => scraper.First(SimpleHtml, "a", sink);

    [Benchmark]
    public int NestedAll() => scraper.Nested(ProductHtml, sink);

    [Benchmark]
    public int WhatwgAllLinks() => scraper.All(specHtml, "a", sink);
}
