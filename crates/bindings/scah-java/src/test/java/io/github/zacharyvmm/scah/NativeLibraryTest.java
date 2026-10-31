package io.github.zacharyvmm.scah;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.IOException;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.attribute.PosixFilePermissions;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class NativeLibraryTest {
    @TempDir
    Path base;

    @Test
    void cachesInAPrivateDirectory() throws IOException {
        assumeTrue(FileSystems.getDefault().supportedFileAttributeViews().contains("posix"));
        NativeLibrary.Cache cache = NativeLibrary.cacheRoot(base);
        assertFalse(cache.temporary);
        assertEquals(base.toAbsolutePath(), cache.root.getParent());
        assertEquals("rwx------", PosixFilePermissions.toString(Files.getPosixFilePermissions(cache.root)));
        assertEquals(cache.root, NativeLibrary.cacheRoot(base).root, "the cache is reused");
    }

    @Test
    void avoidsACacheOthersCanWrite() throws IOException {
        assumeTrue(FileSystems.getDefault().supportedFileAttributeViews().contains("posix"));
        Path shared = NativeLibrary.cacheRoot(base).root;
        Files.setPosixFilePermissions(shared, PosixFilePermissions.fromString("rwxrwxrwx"));

        NativeLibrary.Cache cache = NativeLibrary.cacheRoot(base);
        assertTrue(cache.temporary);
        assertNotEquals(shared, cache.root);
        assertEquals("rwx------", PosixFilePermissions.toString(Files.getPosixFilePermissions(cache.root)));
    }

    @Test
    void avoidsACacheThatIsALink() throws IOException {
        assumeTrue(FileSystems.getDefault().supportedFileAttributeViews().contains("posix"));
        Path shared = NativeLibrary.cacheRoot(base).root;
        Path elsewhere = Files.createDirectory(base.resolve("elsewhere"));
        Files.delete(shared);
        Files.createSymbolicLink(shared, elsewhere);

        assertTrue(NativeLibrary.cacheRoot(base).temporary);
    }

    @Test
    void detectsTheProcessLibc() {
        // CI runs the tests on Alpine for musl and on glibc distributions otherwise.
        assertEquals(Files.exists(Paths.get("/etc/alpine-release")), NativeLibrary.isMusl());
    }
}
