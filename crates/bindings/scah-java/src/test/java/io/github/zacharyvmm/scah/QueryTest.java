package io.github.zacharyvmm.scah;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;
import org.junit.jupiter.api.Test;

class QueryTest {
    @Test
    void parsesAndReadsAttributes() {
        try (Query query = Query.all("a", Save.all()).build();
                Store store = Store.parse("<a href=\"x\">link</a>", query)) {
            List<Element> links = store.get("a");
            assertEquals(1, links.size());
            assertEquals("a", links.get(0).name());
            assertEquals(Optional.of("x"), links.get(0).attribute("href"));
            assertEquals(Optional.of("link"), links.get(0).text());
        }
    }

    @Test
    void chainedSectionsAreReadFromTheirParent() {
        try (Query query = Query.all("main", Save.none()).all("a", Save.all()).build();
                Store store = Store.parse("<main><a href='x'>x</a></main>", query)) {
            Element main = single(store.get("main"));
            assertEquals(Optional.of("x"), single(main.get("a")).attribute("href"));
            assertTrue(store.get("a").isEmpty());
        }
    }

    @Test
    void thenNestsSeveralChildren() {
        try (Query query = Query.all("div", Save.all())
                        .then(Query.all("a", Save.all()), Query.first("span", Save.onlyText()))
                        .build();
                Store store = Store.parse("<div><a href='1'>A</a><span>S</span><span>T</span></div>", query)) {
            Element div = single(store.get("div"));
            assertEquals(Optional.of("1"), single(div.get("a")).attribute("href"));
            assertEquals(Optional.of("S"), single(div.get("span")).text());
        }
    }

    @Test
    void thenNestsDeeply() {
        try (Query query = Query.all("ul", Save.none())
                        .then(Query.all("li", Save.none()).then(List.of(Query.all("a", Save.onlyText()))))
                        .build();
                Store store = Store.parse("<ul><li><a>A</a></li><li><a>B</a><a>C</a></li></ul>", query)) {
            List<List<String>> texts = single(store.get("ul")).get("li").stream()
                    .map(li -> li.get("a").stream()
                            .map(a -> a.text().orElseThrow())
                            .collect(Collectors.toList()))
                    .collect(Collectors.toList());
            assertEquals(List.of(List.of("A"), List.of("B", "C")), texts);
        }
    }

    @Test
    void buildersAreImmutable() {
        QueryBuilder sections = Query.all("section", Save.none());
        String html = "<section><h2>T</h2><a href='/'>L</a></section>";
        try (Query headings = sections.then(Query.first("h2", Save.onlyText())).build();
                Query links = sections.then(Query.all("a", Save.all())).build();
                Query bare = sections.build()) {
            try (Store store = Store.parse(html, headings)) {
                Element section = single(store.get("section"));
                assertEquals(1, section.get("h2").size());
                assertTrue(section.get("a").isEmpty());
            }
            try (Store store = Store.parse(html, links)) {
                Element section = single(store.get("section"));
                assertTrue(section.get("h2").isEmpty());
                assertEquals(1, section.get("a").size());
            }
            try (Store store = Store.parse(html, bare)) {
                assertTrue(single(store.get("section")).get("h2").isEmpty());
            }
        }
    }

    @Test
    void firstStopsAtTheFirstMatch() {
        try (Query query = Query.first("p", Save.onlyText()).build();
                Store store = Store.parse("<p>one</p><p>two</p>", query)) {
            assertEquals(Optional.of("one"), single(store.get("p")).text());
        }
    }

    @Test
    void invalidSelectorsFailToBuild() {
        ScahException error = assertThrows(ScahException.class, () -> Query.all("a[", Save.all()).build());
        assertEquals(ScahStatus.INVALID_SELECTOR, error.status());
        assertFalse(error.getMessage().isEmpty());

        QueryBuilder nested = Query.all("div", Save.all()).then(Query.all("> > a", Save.all()));
        assertEquals(ScahStatus.INVALID_SELECTOR, assertThrows(ScahException.class, nested::build).status());
    }

    @Test
    void argumentsMustNotBeNull() {
        assertThrows(NullPointerException.class, () -> Query.all(null, Save.all()));
        assertThrows(NullPointerException.class, () -> Query.all("a", null));
        assertThrows(NullPointerException.class, () -> Query.all("a", Save.all()).then((QueryBuilder) null));
    }

    @Test
    void savePresetsMatchTheCore() {
        Save all = Save.nameOnly().withInnerHtml(true).withRawText(true).withText(true).withAttributes(true);
        assertEquals(all, Save.all());
        assertEquals(Save.nameOnly().withAttributes(true), Save.none());
        assertEquals(Save.none().withText(true), Save.onlyText());
        assertEquals(Save.none().withRawText(true), Save.onlyRawText());
        assertEquals(Save.none().withInnerHtml(true), Save.onlyInnerHtml());
        assertEquals(Save.nameOnly(), Save.all().withInnerHtml(false).withRawText(false).withText(false)
                .withAttributes(false));
        assertTrue(Save.onlyText().text());
        assertFalse(Save.onlyText().rawText());
    }

    static Element single(List<Element> elements) {
        assertEquals(1, elements.size(), "expected one element");
        return elements.get(0);
    }
}
