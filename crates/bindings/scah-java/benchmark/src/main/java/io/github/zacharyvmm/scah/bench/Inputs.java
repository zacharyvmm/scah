package io.github.zacharyvmm.scah.bench;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/** The documents every library parses; the same ones the Node benchmark generates. */
final class Inputs {
    static final int ELEMENTS = 10_000;

    private Inputs() {}

    /** A flat list of {@code <div class="article"><a>} pairs. */
    static String simple(int count) {
        StringBuilder html = new StringBuilder("<html><body><div id='content'>");
        for (int i = 0; i < count; i++) {
            html.append("<div class=\"article\"><a href=\"/post/").append(i)
                    .append("\"><b>Post</b> &lt;").append(i).append("&gt;</a></div>");
        }
        return html.append("</div></body></html>").toString();
    }

    /** Product cards, each with a title, a rating, and a description. */
    static String products(int count) {
        StringBuilder html = new StringBuilder("<html><body><section id=\"products\">");
        for (int i = 1; i <= count; i++) {
            html.append("<div class=\"product\"><h1>Product #").append(i)
                    .append("</h1><span class=\"rating\">").append((i - 1) % 5 + 1)
                    .append("/5</span><p class=\"description\">Description</p></div>");
        }
        return html.append("</section></body></html>").toString();
    }

    /**
     * The WHATWG HTML specification that {@code just download-html-spec-bench} saves. Set
     * {@code -Dscah.bench.spec=<file>} to read it from elsewhere.
     */
    static String spec() {
        Path path = Paths.get(System.getProperty(
                "scah.bench.spec", "../../../../benches/bench_data/html.spec.whatwg.org.html"));
        try {
            return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
        } catch (IOException e) {
            throw new UncheckedIOException(
                    "missing " + path.toAbsolutePath() + "; run: just download-html-spec-bench", e);
        }
    }
}
