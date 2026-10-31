
namespace Scah.Tests;

public class StoreTests
{
    private const string Listing =
        "<ul id='menu' class='nav'>"
        + "<li><a href='/a' data-x=''>A &amp; <b>b</b></a></li>"
        + "<li><a href='/b' disabled>B</a></li>"
        + "</ul>";

    [Fact]
    public void ReadsEverySavedField()
    {
        using Query query = Query.All("ul", Save.All).Then(Query.All("a", Save.All)).Build();
        using Store store = Store.Parse(Listing, query);

        Element ul = Assert.Single(store.Get("ul"));
        Assert.Equal("ul", ul.Name);
        Assert.Equal("menu", ul.Id);
        Assert.Equal("nav", ul.ClassName);
        Assert.Empty(ul.Attributes);

        ElementList links = ul.Get("a");
        Assert.Equal(2, links.Count);
        Element first = links[0];
        Assert.Equal("A & b", first.Text);
        Assert.Equal("A &amp; b", first.RawText);
        Assert.Equal("A &amp; <b>b</b>", first.InnerHtml);
        Assert.Null(first.Id);
        Assert.Equal("/a", first.GetAttribute("HREF"));
        Assert.Equal("", first.GetAttribute("data-x"));
        Assert.Null(first.GetAttribute("missing"));

        Assert.Null(links[1].GetAttribute("disabled"));
        Assert.Equal([new Attribute("href", "/b"), new Attribute("disabled", null)], links[1].Attributes);
    }

    [Fact]
    public void UnsavedFieldsAreNull()
    {
        using Query query = Query.All("a", Save.NameOnly).Build();
        using Store store = Store.Parse("<a id='i' href='/'>text</a>", query);

        Element link = Assert.Single(store.Get("a"));
        Assert.Equal("a", link.Name);
        Assert.Null(link.Id);
        Assert.Null(link.Text);
        Assert.Null(link.RawText);
        Assert.Null(link.InnerHtml);
        Assert.Null(link.GetAttribute("href"));
        Assert.Empty(link.Attributes);
    }

    [Fact]
    public void QueryResultsTellApartQueriesSharingASelector()
    {
        using Query texts = Query.All("a", Save.OnlyText).Build();
        using Query names = Query.All("a", Save.NameOnly).Build();
        using Query bold = Query.All("b", Save.NameOnly).Build();
        using Store store = Store.Parse("<a>1</a><a>2</a>", texts, names, bold);

        Assert.Equal(["1", "2"], store.QueryResults(0).Select(a => a.Text));
        Assert.Equal([null, null], store.QueryResults(1).Select(a => a.Text));
        Assert.Equal(store.QueryResults(0), store.Get("a"));
        Assert.Empty(store.QueryResults(2));
        Assert.Empty(store.QueryResults(3));
        Assert.Throws<ArgumentOutOfRangeException>(() => store.QueryResults(-1));
        Assert.Equal(4, store.ElementCount);
    }

    [Fact]
    public void MissingSelectorsYieldEmptyLists()
    {
        using Query query = Query.All("ul", Save.None).Then(Query.All("li", Save.None)).Build();
        using Store store = Store.Parse("<ul></ul>", query);

        Assert.Empty(store.Get("never-queried"));
        Element ul = Assert.Single(store.Get("ul"));
        Assert.Empty(ul.Get("li"));
        Assert.Empty(ul.Get("span"));
    }

    [Fact]
    public void ParsesUtf8Bytes()
    {
        using Query query = Query.All("p", Save.OnlyText).Build();
        using Store store = Store.Parse("<p>café ☃ \U0001F600</p>"u8, query);

        Assert.Equal("café ☃ \U0001F600", Assert.Single(store.Get("p")).Text);
    }

    [Fact]
    public void NonAsciiStringsRoundTrip()
    {
        const string Selector = "p[title=\"été\"]";
        using Query query = Query.All(Selector, Save.All).Build();
        using Store store = Store.Parse("<p title=\"été\" déjà=\"vu\">日本</p>", query);

        Element p = Assert.Single(store.Get(Selector));
        Assert.Equal("日本", p.Text);
        Assert.Equal("vu", p.GetAttribute("déjà"));
    }

    [Fact]
    public void LongStringsUseThePool()
    {
        string selector = "a" + string.Concat(Enumerable.Repeat("[x]", 200));
        string html = "<div>" + string.Concat(Enumerable.Repeat("<a x>word </a>", 10_000)) + "</div>";
        using Query query = Query.All(selector, Save.OnlyText).Build();
        using Store store = Store.Parse(html, query);

        Assert.Equal(10_000, store.Get(selector).Count);
    }

    [Fact]
    public void LongNonAsciiStringsKeepTheirPairs()
    {
        string text = new string('x', 10_000) + "\U0001F600 café 日本 " + new string('y', 20_000);
        using Query query = Query.All("p", Save.OnlyText).Build();
        using Store store = Store.Parse("<p>" + text + "</p>", query);

        Assert.Equal(text, Assert.Single(store.Get("p")).Text);
    }

    [Fact]
    public void InvalidUtf8IsRejected()
    {
        using Query query = Query.All("a", Save.All).Build();
        var error = Assert.Throws<ScahException>(() => Store.Parse([(byte)'<', (byte)'a', 0xff], query));
        Assert.Equal(ScahStatus.InvalidUtf8, error.Status);
    }

    [Fact]
    public void LoneSurrogatesAreRejected()
    {
        using Query query = Query.All("a", Save.All).Build();
        var error = Assert.Throws<ScahException>(() => Store.Parse("<a>\ud800</a>", query));
        Assert.Equal(ScahStatus.InvalidUtf16, error.Status);
        Assert.Contains("U+D800 at UTF-16 offset 3", error.Message);
    }

    [Fact]
    public void ParsingNeedsQueries()
    {
        var error = Assert.Throws<ScahException>(() => Store.Parse("<a></a>"));
        Assert.Equal(ScahStatus.EmptyQueries, error.Status);
        Assert.Throws<ArgumentNullException>(() => Store.Parse("<a></a>", [null!]));
    }

    [Fact]
    public void StoresOutliveTheirQueries()
    {
        Query query = Query.All("b", Save.OnlyText).Build();
        Store store = Store.Parse("<b>bold</b>", query);
        query.Dispose();

        Assert.Equal("bold", Assert.Single(store.Get("b")).Text);
        store.Dispose();
    }

    [Fact]
    public void DisposedQueriesCannotParse()
    {
        Query query = Query.All("b", Save.OnlyText).Build();
        query.Dispose();
        Assert.Throws<ObjectDisposedException>(() => Store.Parse("<b></b>", query));
    }

    [Fact]
    public void DisposedStoresCannotBeRead()
    {
        using Query query = Query.All("b", Save.OnlyText).Build();
        Store store = Store.Parse("<b>bold</b>", query);
        Element bold = Assert.Single(store.Get("b"));
        store.Dispose();
        store.Dispose();

        Assert.Throws<ObjectDisposedException>(() => bold.Text);
        Assert.Throws<ObjectDisposedException>(() => store.Get("b"));
    }

    [Fact]
    public void DisposingDuringAReadWaitsForTheRead()
    {
        // Large enough that decoding takes a while after the native call returns.
        string body = new('x', 8 << 20);
        using Query query = Query.All("div", Save.OnlyInnerHtml).Build();
        for (int round = 0; round < 8; round++)
        {
            Store store = Store.Parse($"<div>{body}</div>", query);
            Element div = Assert.Single(store.Get("div"));
            using var reading = new CountdownEvent(4);
            Exception? failure = null;
            Thread[] readers = Enumerable.Range(0, 4).Select(_ => new Thread(() =>
            {
                try
                {
                    reading.Signal();
                    while (true)
                    {
                        if (div.InnerHtml!.Length != body.Length)
                        {
                            throw new InvalidOperationException("Read a truncated string");
                        }
                    }
                }
                catch (ObjectDisposedException)
                {
                    // The store was disposed before this read began.
                }
                catch (Exception e)
                {
                    failure = e;
                }
            })).ToArray();
            foreach (Thread reader in readers)
            {
                reader.Start();
            }
            reading.Wait();
            store.Dispose();
            foreach (Thread reader in readers)
            {
                reader.Join();
            }
            Assert.Null(failure);
        }
    }

    [Fact]
    public void DefaultElementsBelongToNoStore()
    {
        Element element = default;
        Assert.Equal(0, element.Index);
        Assert.Throws<InvalidOperationException>(() => element.Name);
    }

    [Fact]
    public void ElementsCompareByStoreAndRow()
    {
        using Query query = Query.All("a", Save.NameOnly).Build();
        using Store one = Store.Parse("<a></a><a></a>", query);
        using Store two = Store.Parse("<a></a><a></a>", query);

        Assert.Equal(one.Get("a")[0], one.Get("a")[0]);
        Assert.NotEqual(one.Get("a")[0], one.Get("a")[1]);
        Assert.NotEqual(one.Get("a")[0], two.Get("a")[0]);
        Assert.Same(one, one.Get("a")[1].Store);
        Assert.Equal(1, one.Get("a")[1].Index);
    }

    [Fact]
    public void StoresAreReadConcurrently()
    {
        using Query query = Query.All("li", Save.OnlyText).Build();
        string html = string.Concat(Enumerable.Range(0, 1_000).Select(i => $"<li>{i}</li>"));
        using Store store = Store.Parse(html, query);

        Parallel.For(0, 8, _ =>
        {
            ElementList items = store.Get("li");
            for (int i = 0; i < items.Count; i++)
            {
                Assert.Equal(i.ToString(), items[i].Text);
            }
        });
    }

    [Fact]
    public void QueriesAreSharedAcrossThreads()
    {
        using Query query = Query.All("i", Save.OnlyText).Build();
        Parallel.For(0, 64, n =>
        {
            using Store store = Store.Parse($"<i>{n}</i>", query);
            Assert.Equal(n.ToString(), Assert.Single(store.Get("i")).Text);
        });
    }

    [Fact]
    public void FinalizersFreeAbandonedHandles()
    {
        for (int i = 0; i < 1_000; i++)
        {
            Store.Parse("<a>x</a>", Query.All("a", Save.All).Build()).Get("a");
        }
        GC.Collect();
        GC.WaitForPendingFinalizers();
    }
}
