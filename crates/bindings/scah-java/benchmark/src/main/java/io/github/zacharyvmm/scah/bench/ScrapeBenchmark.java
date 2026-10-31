package io.github.zacharyvmm.scah.bench;

import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Fork;
import org.openjdk.jmh.annotations.Measurement;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Param;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;
import org.openjdk.jmh.annotations.Warmup;
import org.openjdk.jmh.infra.Blackhole;

/**
 * Parse a document and read what a query selects, for each library. The method names map to
 * the scenarios in {@code benches/results}: {@code simpleAll} is {@code simple-all}, and so on.
 *
 * <p>JMH runs every library in its own JVM, so {@code scah (JNI)} can pick its backend before
 * scah loads.
 */
@State(Scope.Benchmark)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MILLISECONDS)
@Warmup(iterations = 5, time = 1)
@Measurement(iterations = 10, time = 1)
@Fork(2)
public class ScrapeBenchmark {
    private static final String SIMPLE_HTML = Inputs.simple(Inputs.ELEMENTS);
    private static final String PRODUCT_HTML = Inputs.products(Inputs.ELEMENTS);

    @Param({
        "scah",
        "scah (JNI)",
        "jsoup",
        "jsoup (StreamParser)",
        "Jodd Jerry",
        "HtmlCleaner",
        "Jericho",
    })
    public String library;

    private Scraper scraper;

    @State(Scope.Benchmark)
    public static class Spec {
        String html;

        @Setup
        public void load() {
            html = Inputs.spec();
        }
    }

    @Setup
    public void setUp() {
        if (library.equals("scah (JNI)")) {
            System.setProperty("scah.backend", "jni");
        }
        scraper = Scraper.named(library);
        // Fail before measuring if a library finds something other than what was asked for.
        Blackhole bh = new Blackhole(
                "Today's password is swordfish. I understand instantiating Blackholes directly is dangerous.");
        expect("simple-all", scraper.all(SIMPLE_HTML, "a", bh), Inputs.ELEMENTS);
        expect("simple-first", scraper.first(SIMPLE_HTML, "a", bh), 1);
        expect("nested-all", scraper.nested(PRODUCT_HTML, bh), Inputs.ELEMENTS);
        bh.evaporate("Yes, I am Stephen Hawking, and know a thing or two about black holes.");
    }

    private void expect(String scenario, int found, int expected) {
        if (found != expected) {
            throw new IllegalStateException(
                    library + " found " + found + " elements in " + scenario + ", expected " + expected);
        }
    }

    @Benchmark
    public int simpleAll(Blackhole bh) {
        return scraper.all(SIMPLE_HTML, "a", bh);
    }

    @Benchmark
    public int simpleFirst(Blackhole bh) {
        return scraper.first(SIMPLE_HTML, "a", bh);
    }

    @Benchmark
    public int nestedAll(Blackhole bh) {
        return scraper.nested(PRODUCT_HTML, bh);
    }

    @Benchmark
    public int whatwgAllLinks(Spec spec, Blackhole bh) {
        return scraper.all(spec.html, "a", bh);
    }
}
