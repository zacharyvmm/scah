namespace Scah.Tests;

public class QueryTests
{
    [Fact]
    public void ParsesAndReadsAttributes()
    {
        using Query query = Query.All("a", Save.All).Build();
        using Store store = Store.Parse("<a href=\"x\">link</a>", query);

        ElementList links = store.Get("a");
        Assert.Single(links);
        Assert.Equal("a", links[0].Name);
        Assert.Equal("x", links[0].GetAttribute("href"));
        Assert.Equal("link", links[0].Text);
    }

    [Fact]
    public void ChainedSectionsAreReadFromTheirParent()
    {
        using Query query = Query.All("main", Save.None).All("a", Save.All).Build();
        using Store store = Store.Parse("<main><a href='x'>x</a></main>", query);

        Element main = Assert.Single(store.Get("main"));
        Element link = Assert.Single(main.Get("a"));
        Assert.Equal("x", link.GetAttribute("href"));
        Assert.Empty(store.Get("a"));
    }

    [Fact]
    public void ThenNestsSeveralChildren()
    {
        using Query query = Query.All("div", Save.All)
            .Then(Query.All("a", Save.All), Query.First("span", Save.OnlyText))
            .Build();
        using Store store = Store.Parse("<div><a href='1'>A</a><span>S</span><span>T</span></div>", query);

        Element div = Assert.Single(store.Get("div"));
        Assert.Equal("1", Assert.Single(div.Get("a")).GetAttribute("href"));
        Assert.Equal("S", Assert.Single(div.Get("span")).Text);
    }

    [Fact]
    public void ThenNestsDeeply()
    {
        using Query query = Query.All("ul", Save.None)
            .Then(Query.All("li", Save.None).Then(Query.All("a", Save.OnlyText)))
            .Build();
        using Store store = Store.Parse(
            "<ul><li><a>A</a></li><li><a>B</a><a>C</a></li></ul>", query);

        Element ul = Assert.Single(store.Get("ul"));
        string[][] texts = ul.Get("li")
            .Select(li => li.Get("a").Select(a => a.Text!).ToArray())
            .ToArray();
        Assert.Equal([["A"], ["B", "C"]], texts);
    }

    [Fact]
    public void BuildersAreImmutable()
    {
        QueryBuilder sections = Query.All("section", Save.None);
        using Query headings = sections.Then(Query.First("h2", Save.OnlyText)).Build();
        using Query links = sections.Then(Query.All("a", Save.All)).Build();
        using Query bare = sections.Build();
        const string Html = "<section><h2>T</h2><a href='/'>L</a></section>";

        using (Store store = Store.Parse(Html, headings))
        {
            Element section = Assert.Single(store.Get("section"));
            Assert.Single(section.Get("h2"));
            Assert.Empty(section.Get("a"));
        }
        using (Store store = Store.Parse(Html, links))
        {
            Element section = Assert.Single(store.Get("section"));
            Assert.Empty(section.Get("h2"));
            Assert.Single(section.Get("a"));
        }
        using (Store store = Store.Parse(Html, bare))
        {
            Assert.Empty(Assert.Single(store.Get("section")).Get("h2"));
        }
    }

    [Fact]
    public void FirstStopsAtTheFirstMatch()
    {
        using Query query = Query.First("p", Save.OnlyText).Build();
        using Store store = Store.Parse("<p>one</p><p>two</p>", query);

        Assert.Equal("one", Assert.Single(store.Get("p")).Text);
    }

    [Fact]
    public void InvalidSelectorsFailToBuild()
    {
        var error = Assert.Throws<ScahException>(() => Query.All("a[", Save.All).Build());
        Assert.Equal(ScahStatus.InvalidSelector, error.Status);
        Assert.NotEmpty(error.Message);

        QueryBuilder nested = Query.All("div", Save.All).Then(Query.All("> > a", Save.All));
        Assert.Equal(ScahStatus.InvalidSelector, Assert.Throws<ScahException>(nested.Build).Status);
    }

    [Fact]
    public void SelectorsMustBeStrings()
    {
        Assert.Throws<ArgumentNullException>(() => Query.All(null!, Save.All));
        Assert.Throws<ArgumentNullException>(() => Query.All("a", Save.All).Then(null!, null!));
    }

    [Fact]
    public void SavePresetsMatchTheCore()
    {
        Assert.Equal(new Save { InnerHtml = true, RawText = true, Text = true, Attributes = true }, Save.All);
        Assert.Equal(new Save { Attributes = true }, Save.None);
        Assert.Equal(default, Save.NameOnly);
        Assert.Equal(new Save { Text = true, Attributes = true }, Save.OnlyText);
        Assert.Equal(new Save { RawText = true, Attributes = true }, Save.OnlyRawText);
        Assert.Equal(new Save { InnerHtml = true, Attributes = true }, Save.OnlyInnerHtml);
    }
}
