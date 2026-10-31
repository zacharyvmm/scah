using System.Text;

namespace Scah.Benchmarks;

/// <summary>The documents every library parses; the same ones the Node benchmark generates.</summary>
internal static class Inputs
{
    public const int Elements = 10_000;

    /// <summary>A flat list of <c>&lt;div class="article"&gt;&lt;a&gt;</c> pairs.</summary>
    public static string Simple(int count)
    {
        var html = new StringBuilder("<html><body><div id='content'>");
        for (var i = 0; i < count; i++)
        {
            html.Append($"<div class=\"article\"><a href=\"/post/{i}\"><b>Post</b> &lt;{i}&gt;</a></div>");
        }
        return html.Append("</div></body></html>").ToString();
    }

    /// <summary>Product cards, each with a title, a rating, and a description.</summary>
    public static string Products(int count)
    {
        var html = new StringBuilder("<html><body><section id=\"products\">");
        for (var i = 1; i <= count; i++)
        {
            html.Append($"<div class=\"product\"><h1>Product #{i}</h1><span class=\"rating\">{(i - 1) % 5 + 1}/5</span><p class=\"description\">Description</p></div>");
        }
        return html.Append("</section></body></html>").ToString();
    }

    /// <summary>
    /// The WHATWG HTML specification that <c>just download-html-spec-bench</c> saves. Set
    /// <c>SCAH_BENCH_SPEC</c> to read it from elsewhere.
    /// </summary>
    public static string Spec()
    {
        var path = Environment.GetEnvironmentVariable("SCAH_BENCH_SPEC")
            ?? Path.Combine(RepositoryRoot(), "benches", "bench_data", "html.spec.whatwg.org.html");
        if (!File.Exists(path))
        {
            throw new FileNotFoundException($"missing {path}; run: just download-html-spec-bench", path);
        }
        return File.ReadAllText(path);
    }

    // BenchmarkDotNet runs each benchmark from its own build directory, so look upward.
    private static string RepositoryRoot()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "Cargo.lock")))
            {
                return dir.FullName;
            }
        }
        throw new DirectoryNotFoundException($"no Cargo.lock above {AppContext.BaseDirectory}");
    }
}
