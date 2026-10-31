package io.github.zacharyvmm.scah.bench;

import io.github.zacharyvmm.scah.Element;
import io.github.zacharyvmm.scah.Query;
import io.github.zacharyvmm.scah.Save;
import io.github.zacharyvmm.scah.Store;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.util.List;
import jodd.jerry.Jerry;
import net.htmlparser.jericho.Config;
import net.htmlparser.jericho.LoggerProvider;
import net.htmlparser.jericho.Source;
import org.htmlcleaner.HtmlCleaner;
import org.htmlcleaner.TagNode;
import org.jsoup.Jsoup;
import org.jsoup.nodes.Document;
import org.jsoup.parser.Parser;
import org.jsoup.parser.StreamParser;
import org.openjdk.jmh.infra.Blackhole;

/**
 * One way per library to parse a document and read what it selects.
 *
 * <p>Every scraper reads the inner HTML, the text, and the {@code href} of each element it
 * finds, and returns how many it found. The nested scenario reads every product card and its
 * title, rating, and description.
 */
interface Scraper {
    int all(String html, String selector, Blackhole bh);

    int first(String html, String selector, Blackhole bh);

    int nested(String html, Blackhole bh);

    static Scraper named(String name) {
        switch (name) {
            case "scah":
            case "scah (JNI)":
                return new ScahScraper();
            case "jsoup":
                return new JsoupScraper();
            case "jsoup (StreamParser)":
                return new JsoupStreamScraper();
            case "Jodd Jerry":
                return new JerryScraper();
            case "HtmlCleaner":
                return new HtmlCleanerScraper();
            case "Jericho":
                return new JerichoScraper();
            default:
                throw new IllegalArgumentException("unknown library " + name);
        }
    }

    final class ScahScraper implements Scraper {
        private static final Save SAVE = Save.onlyInnerHtml().withText(true);

        @Override
        public int all(String html, String selector, Blackhole bh) {
            try (Query query = Query.all(selector, SAVE).build(); Store store = Store.parse(html, query)) {
                List<Element> elements = store.get(selector);
                for (Element element : elements) {
                    read(element, bh);
                }
                return elements.size();
            }
        }

        @Override
        public int first(String html, String selector, Blackhole bh) {
            try (Query query = Query.first(selector, SAVE).build(); Store store = Store.parse(html, query)) {
                List<Element> elements = store.get(selector);
                if (elements.isEmpty()) {
                    return 0;
                }
                read(elements.get(0), bh);
                return 1;
            }
        }

        @Override
        public int nested(String html, Blackhole bh) {
            try (Query query = Query.all(".product", SAVE)
                            .then(Query.first("> h1", SAVE),
                                    Query.first("> .rating", SAVE),
                                    Query.first("> .description", SAVE))
                            .build();
                    Store store = Store.parse(html, query)) {
                List<Element> products = store.get(".product");
                for (Element product : products) {
                    read(product, bh);
                    read(product.get("> h1").get(0), bh);
                    read(product.get("> .rating").get(0), bh);
                    read(product.get("> .description").get(0), bh);
                }
                return products.size();
            }
        }

        private static void read(Element element, Blackhole bh) {
            bh.consume(element.innerHtml().orElse(null));
            bh.consume(element.text().orElse(null));
            bh.consume(element.attribute("href").orElse(null));
        }
    }

    final class JsoupScraper implements Scraper {
        @Override
        public int all(String html, String selector, Blackhole bh) {
            Document document = Jsoup.parse(html);
            int count = 0;
            for (org.jsoup.nodes.Element element : document.select(selector)) {
                read(element, bh);
                count++;
            }
            return count;
        }

        @Override
        public int first(String html, String selector, Blackhole bh) {
            org.jsoup.nodes.Element element = Jsoup.parse(html).selectFirst(selector);
            if (element == null) {
                return 0;
            }
            read(element, bh);
            return 1;
        }

        @Override
        public int nested(String html, Blackhole bh) {
            int count = 0;
            for (org.jsoup.nodes.Element product : Jsoup.parse(html).select(".product")) {
                read(product, bh);
                read(product.selectFirst("h1"), bh);
                read(product.selectFirst(".rating"), bh);
                read(product.selectFirst(".description"), bh);
                count++;
            }
            return count;
        }

        static void read(org.jsoup.nodes.Element element, Blackhole bh) {
            bh.consume(element.html());
            bh.consume(element.text());
            bh.consume(element.attr("href"));
        }
    }

    /**
     * jsoup's streaming parser, which builds the tree only as far as the next match. Elements
     * are removed once read, so the tree stays small.
     */
    final class JsoupStreamScraper implements Scraper {
        @Override
        public int all(String html, String selector, Blackhole bh) {
            try (StreamParser parser = new StreamParser(Parser.htmlParser()).parse(html, "")) {
                int count = 0;
                org.jsoup.nodes.Element element;
                while ((element = parser.selectNext(selector)) != null) {
                    JsoupScraper.read(element, bh);
                    element.remove();
                    count++;
                }
                return count;
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }

        @Override
        public int first(String html, String selector, Blackhole bh) {
            try (StreamParser parser = new StreamParser(Parser.htmlParser()).parse(html, "")) {
                org.jsoup.nodes.Element element = parser.selectFirst(selector);
                if (element == null) {
                    return 0;
                }
                JsoupScraper.read(element, bh);
                return 1;
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }

        @Override
        public int nested(String html, Blackhole bh) {
            try (StreamParser parser = new StreamParser(Parser.htmlParser()).parse(html, "")) {
                int count = 0;
                org.jsoup.nodes.Element product;
                while ((product = parser.selectNext(".product")) != null) {
                    JsoupScraper.read(product, bh);
                    JsoupScraper.read(product.selectFirst("h1"), bh);
                    JsoupScraper.read(product.selectFirst(".rating"), bh);
                    JsoupScraper.read(product.selectFirst(".description"), bh);
                    product.remove();
                    count++;
                }
                return count;
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
    }

    final class JerryScraper implements Scraper {
        @Override
        public int all(String html, String selector, Blackhole bh) {
            Jerry elements = Jerry.of(html).s(selector);
            for (Jerry element : elements) {
                read(element, bh);
            }
            return elements.size();
        }

        @Override
        public int first(String html, String selector, Blackhole bh) {
            Jerry element = Jerry.of(html).s(selector).first();
            if (element.size() == 0) {
                return 0;
            }
            read(element, bh);
            return 1;
        }

        @Override
        public int nested(String html, Blackhole bh) {
            Jerry products = Jerry.of(html).s(".product");
            for (Jerry product : products) {
                read(product, bh);
                read(product.find("h1").first(), bh);
                read(product.find(".rating").first(), bh);
                read(product.find(".description").first(), bh);
            }
            return products.size();
        }

        private static void read(Jerry element, Blackhole bh) {
            bh.consume(element.html());
            bh.consume(element.text());
            bh.consume(element.attr("href"));
        }
    }

    /** HtmlCleaner has no CSS selectors; it finds elements by tag name or attribute value. */
    final class HtmlCleanerScraper implements Scraper {
        private final HtmlCleaner cleaner = new HtmlCleaner();

        @Override
        public int all(String html, String tag, Blackhole bh) {
            TagNode[] elements = cleaner.clean(html).getElementsByName(tag, true);
            for (TagNode element : elements) {
                read(element, bh);
            }
            return elements.length;
        }

        @Override
        public int first(String html, String tag, Blackhole bh) {
            TagNode element = cleaner.clean(html).findElementByName(tag, true);
            if (element == null) {
                return 0;
            }
            read(element, bh);
            return 1;
        }

        @Override
        public int nested(String html, Blackhole bh) {
            TagNode[] products = cleaner.clean(html).getElementsByAttValue("class", "product", true, false);
            for (TagNode product : products) {
                read(product, bh);
                read(product.findElementByName("h1", false), bh);
                read(product.findElementByAttValue("class", "rating", false, false), bh);
                read(product.findElementByAttValue("class", "description", false, false), bh);
            }
            return products.length;
        }

        private void read(TagNode element, Blackhole bh) {
            bh.consume(cleaner.getInnerHtml(element));
            bh.consume(element.getText().toString());
            bh.consume(element.getAttributeByName("href"));
        }
    }

    /**
     * Jericho has no CSS selectors either; it finds elements by tag name or class. It parses
     * lazily, so the first-match scenario reads only the start of the document.
     */
    final class JerichoScraper implements Scraper {
        static {
            Config.LoggerProvider = LoggerProvider.DISABLED;
        }

        @Override
        public int all(String html, String tag, Blackhole bh) {
            Source source = new Source(html);
            source.fullSequentialParse();
            List<net.htmlparser.jericho.Element> elements = source.getAllElements(tag);
            for (net.htmlparser.jericho.Element element : elements) {
                read(element, bh);
            }
            return elements.size();
        }

        @Override
        public int first(String html, String tag, Blackhole bh) {
            net.htmlparser.jericho.Element element = new Source(html).getFirstElement(tag);
            if (element == null) {
                return 0;
            }
            read(element, bh);
            return 1;
        }

        @Override
        public int nested(String html, Blackhole bh) {
            Source source = new Source(html);
            source.fullSequentialParse();
            List<net.htmlparser.jericho.Element> products = source.getAllElementsByClass("product");
            for (net.htmlparser.jericho.Element product : products) {
                read(product, bh);
                read(product.getFirstElement("h1"), bh);
                read(product.getFirstElementByClass("rating"), bh);
                read(product.getFirstElementByClass("description"), bh);
            }
            return products.size();
        }

        private static void read(net.htmlparser.jericho.Element element, Blackhole bh) {
            bh.consume(element.getContent().toString());
            bh.consume(element.getTextExtractor().toString());
            bh.consume(element.getAttributeValue("href"));
        }
    }
}
