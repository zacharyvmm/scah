package io.github.zacharyvmm.scah;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ClassLoaderTest {
    @Test
    void independentClassLoadersEachLoadTheBinding() throws Exception {
        URL jar = Query.class.getProtectionDomain().getCodeSource().getLocation();
        for (int i = 0; i < 2; i++) {
            try (URLClassLoader loader = new URLClassLoader(new URL[] {jar}, ClassLoader.getPlatformClassLoader())) {
                assertEquals(1, parseLinks(loader));
            }
        }
        // And this test's own classloader still works.
        try (Query query = Query.all("a", Save.all()).build(); Store store = Store.parse("<a></a>", query)) {
            assertEquals(1, store.get("a").size());
        }
    }

    @Test
    void copiesForOtherClassLoadersGoInTheCache(@TempDir Path cache)
            throws Exception {
        String previous = System.getProperty("scah.library.cache");
        System.setProperty("scah.library.cache", cache.toString());
        try {
            Path directory = NativeLibrary.privateDirectory("jni-");
            assertEquals(NativeLibrary.cacheRoot(cache).root, directory.getParent());
        } finally {
            if (previous == null) {
                System.clearProperty("scah.library.cache");
            } else {
                System.setProperty("scah.library.cache", previous);
            }
        }
    }

    /** Parse a link with the binding's classes from {@code loader}. */
    private static int parseLinks(ClassLoader loader) throws Exception {
        Class<?> save = Class.forName("io.github.zacharyvmm.scah.Save", true, loader);
        Class<?> query = Class.forName("io.github.zacharyvmm.scah.Query", true, loader);
        Class<?> builder = Class.forName("io.github.zacharyvmm.scah.QueryBuilder", true, loader);
        Class<?> store = Class.forName("io.github.zacharyvmm.scah.Store", true, loader);

        Object all = save.getMethod("all").invoke(null);
        Object built = builder.getMethod("build")
                .invoke(query.getMethod("all", String.class, save).invoke(null, "a", all));
        Object queries = Array.newInstance(query, 1);
        Array.set(queries, 0, built);
        Method parse = store.getMethod("parse", String.class, queries.getClass());
        Object parsed = parse.invoke(null, "<a href='/'>x</a>", queries);
        try {
            return ((List<?>) store.getMethod("get", String.class).invoke(parsed, "a")).size();
        } finally {
            store.getMethod("close").invoke(parsed);
            query.getMethod("close").invoke(built);
        }
    }
}
