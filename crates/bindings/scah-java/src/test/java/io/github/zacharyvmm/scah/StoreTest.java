package io.github.zacharyvmm.scah;

import static io.github.zacharyvmm.scah.QueryTest.single;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import org.junit.jupiter.api.Test;

class StoreTest {
    private static final String LISTING = "<ul id='menu' class='nav'>"
            + "<li><a href='/a' data-x=''>A &amp; <b>b</b></a></li>"
            + "<li><a href='/b' disabled>B</a></li>"
            + "</ul>";

    @Test
    void readsEverySavedField() {
        try (Query query = Query.all("ul", Save.all()).then(Query.all("a", Save.all())).build();
                Store store = Store.parse(LISTING, query)) {
            Element ul = single(store.get("ul"));
            assertEquals("ul", ul.name());
            assertEquals(Optional.of("menu"), ul.id());
            assertEquals(Optional.of("nav"), ul.className());
            assertTrue(ul.attributes().isEmpty());

            List<Element> links = ul.get("a");
            assertEquals(2, links.size());
            Element first = links.get(0);
            assertEquals(Optional.of("A & b"), first.text());
            assertEquals(Optional.of("A &amp; b"), first.rawText());
            assertEquals(Optional.of("A &amp; <b>b</b>"), first.innerHtml());
            assertEquals(Optional.empty(), first.id());
            assertEquals(Optional.of("/a"), first.attribute("HREF"));
            assertEquals(Optional.of(""), first.attribute("data-x"));
            assertEquals(Optional.empty(), first.attribute("missing"));

            assertEquals(Optional.empty(), links.get(1).attribute("disabled"));
            List<Attribute> attributes = links.get(1).attributes();
            assertEquals(List.of(new Attribute("href", "/b"), new Attribute("disabled", null)), attributes);
            assertEquals(Optional.empty(), attributes.get(1).value());
        }
    }

    @Test
    void unsavedFieldsAreEmpty() {
        try (Query query = Query.all("a", Save.nameOnly()).build();
                Store store = Store.parse("<a id='i' href='/'>text</a>", query)) {
            Element link = single(store.get("a"));
            assertEquals("a", link.name());
            assertEquals(Optional.empty(), link.id());
            assertEquals(Optional.empty(), link.text());
            assertEquals(Optional.empty(), link.rawText());
            assertEquals(Optional.empty(), link.innerHtml());
            assertEquals(Optional.empty(), link.attribute("href"));
            assertTrue(link.attributes().isEmpty());
        }
    }

    @Test
    void queryResultsTellApartQueriesSharingASelector() {
        try (Query texts = Query.all("a", Save.onlyText()).build();
                Query names = Query.all("a", Save.nameOnly()).build();
                Query bold = Query.all("b", Save.nameOnly()).build();
                Store store = Store.parse("<a>1</a><a>2</a>", texts, names, bold)) {
            assertEquals(List.of(Optional.of("1"), Optional.of("2")), texts(store.queryResults(0)));
            assertEquals(List.of(Optional.empty(), Optional.empty()), texts(store.queryResults(1)));
            assertEquals(store.queryResults(0), store.get("a"));
            assertTrue(store.queryResults(2).isEmpty());
            assertTrue(store.queryResults(3).isEmpty());
            assertThrows(IndexOutOfBoundsException.class, () -> store.queryResults(-1));
            assertEquals(4, store.elementCount());
        }
    }

    @Test
    void missingSelectorsYieldEmptyLists() {
        try (Query query = Query.all("ul", Save.none()).then(Query.all("li", Save.none())).build();
                Store store = Store.parse("<ul></ul>", query)) {
            assertTrue(store.get("never-queried").isEmpty());
            Element ul = single(store.get("ul"));
            assertTrue(ul.get("li").isEmpty());
            assertTrue(ul.get("span").isEmpty());
        }
    }

    @Test
    void parsesUtf8Bytes() {
        byte[] html = "<p>café ☃ 😀</p>".getBytes(StandardCharsets.UTF_8);
        try (Query query = Query.all("p", Save.onlyText()).build(); Store store = Store.parse(html, query)) {
            assertEquals(Optional.of("café ☃ 😀"), single(store.get("p")).text());
        }
    }

    @Test
    void nonAsciiStringsRoundTrip() {
        String selector = "p[title=\"été\"]";
        try (Query query = Query.all(selector, Save.all()).build();
                Store store = Store.parse("<p title=\"été\" déjà=\"vu\">日本</p>", query)) {
            Element p = single(store.get(selector));
            assertEquals(Optional.of("日本"), p.text());
            assertEquals(Optional.of("vu"), p.attribute("déjà"));
        }
    }

    @Test
    void longStringsSpillOutOfTheScratchArea() {
        String selector = "a" + "[x]".repeat(200);
        String html = "<div>" + "<a x>word </a>".repeat(10_000) + "</div>";
        try (Query query = Query.all(selector, Save.onlyText()).build(); Store store = Store.parse(html, query)) {
            assertEquals(10_000, store.get(selector).size());
        }
    }

    @Test
    void stringsSpanningSeveralBlocksKeepTheirPairs() {
        // The backends hand the library 8,192 UTF-16 units at a time. Put a
        // surrogate pair across that boundary, between long ASCII runs.
        String text = "x".repeat(8_192 - 4) + "😀 café 日本 " + "y".repeat(20_000);
        String html = "<p>" + text + "</p>";
        assertEquals('\ud83d', html.charAt(8_191));
        try (Query query = Query.all("p", Save.onlyText()).build(); Store store = Store.parse(html, query)) {
            assertEquals(Optional.of(text), single(store.get("p")).text());
        }
    }

    @Test
    void loneSurrogatesBecomeReplacementCharacters() {
        try (Query query = Query.all("a", Save.onlyText()).build();
                Store store = Store.parse("<a>\ud800x\udc00</a><a>end\ud83d</a>", query)) {
            assertEquals(List.of(Optional.of("\ufffdx\ufffd"), Optional.of("end\ufffd")), texts(store.get("a")));
        }
    }

    @Test
    void invalidUtf8IsRejected() {
        try (Query query = Query.all("a", Save.all()).build()) {
            ScahException error = assertThrows(
                    ScahException.class, () -> Store.parse(new byte[] {'<', 'a', (byte) 0xff}, query));
            assertEquals(ScahStatus.INVALID_UTF8, error.status());
        }
    }

    @Test
    void parsingNeedsQueries() {
        ScahException error = assertThrows(ScahException.class, () -> Store.parse("<a></a>"));
        assertEquals(ScahStatus.EMPTY_QUERIES, error.status());
        assertThrows(NullPointerException.class, () -> Store.parse("<a></a>", (Query) null));
    }

    @Test
    void storesOutliveTheirQueries() {
        Query query = Query.all("b", Save.onlyText()).build();
        try (Store store = Store.parse("<b>bold</b>", query)) {
            query.close();
            assertEquals(Optional.of("bold"), single(store.get("b")).text());
        }
    }

    @Test
    void closedQueriesCannotParse() {
        Query query = Query.all("b", Save.onlyText()).build();
        query.close();
        query.close();
        assertThrows(IllegalStateException.class, () -> Store.parse("<b></b>", query));
    }

    @Test
    void closedStoresCannotBeRead() {
        try (Query query = Query.all("b", Save.onlyText()).build()) {
            Store store = Store.parse("<b>bold</b>", query);
            Element bold = single(store.get("b"));
            store.close();
            store.close();
            assertThrows(IllegalStateException.class, bold::text);
            assertThrows(IllegalStateException.class, () -> store.get("b"));
        }
    }

    @Test
    void elementsCompareByStoreAndRow() {
        try (Query query = Query.all("a", Save.nameOnly()).build();
                Store one = Store.parse("<a></a><a></a>", query);
                Store two = Store.parse("<a></a><a></a>", query)) {
            assertEquals(one.get("a").get(0), one.get("a").get(0));
            assertEquals(one.get("a").get(0).hashCode(), one.get("a").get(0).hashCode());
            assertNotEquals(one.get("a").get(0), one.get("a").get(1));
            assertNotEquals(one.get("a").get(0), two.get("a").get(0));
            assertSame(one, one.get("a").get(1).store());
            assertEquals(1, one.get("a").get(1).index());
        }
    }

    @Test
    void storesAreReadConcurrently() throws Exception {
        String html = IntStream.range(0, 1_000).mapToObj(i -> "<li>" + i + "</li>").collect(Collectors.joining());
        ExecutorService pool = Executors.newFixedThreadPool(8);
        try (Query query = Query.all("li", Save.onlyText()).build(); Store store = Store.parse(html, query)) {
            List<Future<?>> tasks = new ArrayList<>();
            for (int t = 0; t < 8; t++) {
                tasks.add(pool.submit(() -> {
                    List<Element> items = store.get("li");
                    for (int i = 0; i < items.size(); i++) {
                        assertEquals(Optional.of(Integer.toString(i)), items.get(i).text());
                    }
                }));
            }
            for (Future<?> task : tasks) {
                task.get();
            }
        } finally {
            pool.shutdown();
        }
    }

    @Test
    void closingWhileOtherThreadsReadIsSafe() throws Exception {
        String html = "<li>x</li>".repeat(1_000);
        ExecutorService pool = Executors.newFixedThreadPool(4);
        try (Query query = Query.all("li", Save.onlyText()).build()) {
            for (int round = 0; round < 50; round++) {
                Store store = Store.parse(html, query);
                List<Element> items = store.get("li");
                List<Future<?>> tasks = new ArrayList<>();
                for (int t = 0; t < 4; t++) {
                    tasks.add(pool.submit(() -> {
                        try {
                            for (Element item : items) {
                                assertEquals(Optional.of("x"), item.text());
                            }
                        } catch (IllegalStateException closed) {
                            // The store was closed under this reader, which is the expected outcome.
                        }
                    }));
                }
                store.close();
                for (Future<?> task : tasks) {
                    task.get();
                }
            }
        } finally {
            pool.shutdown();
        }
    }

    @Test
    void cleanersFreeAbandonedHandles() {
        for (int i = 0; i < 1_000; i++) {
            Store.parse("<a>x</a>", Query.all("a", Save.all()).build()).get("a");
        }
        System.gc();
    }

    @Test
    void platformIsDetected() {
        String rid = NativeLibrary.runtimeIdentifier();
        assertTrue(rid.matches("(linux|linux-musl|osx|win)-(x64|arm64)"), rid);
    }

    @Test
    void elementListsAreReadOnly() {
        try (Query query = Query.all("a", Save.nameOnly()).build(); Store store = Store.parse("<a></a>", query)) {
            List<Element> links = store.get("a");
            assertThrows(UnsupportedOperationException.class, () -> links.remove(0));
            assertArrayEquals(new Object[] {links.get(0)}, links.toArray());
        }
    }

    private static List<Optional<String>> texts(List<Element> elements) {
        return elements.stream().map(Element::text).collect(Collectors.toList());
    }
}
