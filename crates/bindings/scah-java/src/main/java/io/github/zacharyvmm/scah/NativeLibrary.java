package io.github.zacharyvmm.scah;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;

/**
 * Finds a native library on disk.
 *
 * <p>When the {@code scah.library.dir} system property names a directory, the library is
 * loaded from there. Otherwise it is extracted from the jar, under {@code
 * io/github/zacharyvmm/scah/native/<rid>/}, into a cache directory named after its checksum:
 * {@code scah.library.cache} if set, else {@code java.io.tmpdir}. Extraction is atomic, so
 * concurrent JVMs share one copy.
 */
final class NativeLibrary {
    private static final String RESOURCES = "/io/github/zacharyvmm/scah/native/";

    private NativeLibrary() {}

    /** The path of the library {@code name}, such as {@code scah_c}. */
    static Path locate(String name) {
        String file = System.mapLibraryName(name);
        String dir = System.getProperty("scah.library.dir", "");
        if (!dir.isEmpty()) {
            Path path = Paths.get(dir, file);
            if (!Files.isRegularFile(path)) {
                throw new UnsatisfiedLinkError("scah.library.dir has no " + file + ": " + path);
            }
            return path.toAbsolutePath();
        }

        String rid = runtimeIdentifier();
        String resource = RESOURCES + rid + "/" + file;
        byte[] library;
        try (InputStream stream = NativeLibrary.class.getResourceAsStream(resource)) {
            if (stream == null) {
                throw new UnsatisfiedLinkError(
                        "scah has no native library for " + rid + " (looked for " + resource + ")");
            }
            library = stream.readAllBytes();
        } catch (IOException e) {
            throw new UncheckedIOException("Cannot read " + resource, e);
        }

        String cache = System.getProperty("scah.library.cache", System.getProperty("java.io.tmpdir"));
        Path target = Paths.get(cache, "scah-" + checksum(library), rid, file);
        try {
            if (Files.isRegularFile(target) && Files.size(target) == library.length) {
                return target;
            }
            Files.createDirectories(target.getParent());
            Path temporary = Files.createTempFile(target.getParent(), file, ".tmp");
            try {
                Files.write(temporary, library);
                try {
                    Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE);
                } catch (AtomicMoveNotSupportedException e) {
                    Files.move(temporary, target);
                }
            } catch (FileAlreadyExistsException e) {
                // Another JVM extracted it first; its copy is identical.
            } finally {
                Files.deleteIfExists(temporary);
            }
            return target;
        } catch (IOException e) {
            // Another JVM may have the library open on Windows, which forbids replacing it.
            if (Files.isRegularFile(target)) {
                return target;
            }
            throw new UncheckedIOException("Cannot extract " + resource + " to " + target, e);
        }
    }

    /** The .NET runtime identifier of this platform, which names the library's resource folder. */
    static String runtimeIdentifier() {
        String os = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch").toLowerCase(Locale.ROOT);
        String cpu;
        if (arch.equals("amd64") || arch.equals("x86_64")) {
            cpu = "x64";
        } else if (arch.equals("aarch64") || arch.equals("arm64")) {
            cpu = "arm64";
        } else {
            throw new UnsatisfiedLinkError("scah has no native library for architecture " + arch);
        }
        if (os.startsWith("windows")) {
            return "win-" + cpu;
        }
        if (os.startsWith("mac")) {
            return "osx-" + cpu;
        }
        if (os.startsWith("linux")) {
            return (isMusl() ? "linux-musl-" : "linux-") + cpu;
        }
        throw new UnsatisfiedLinkError("scah has no native library for " + os);
    }

    private static boolean isMusl() {
        try (var entries = Files.newDirectoryStream(Paths.get("/lib"), "ld-musl-*")) {
            return entries.iterator().hasNext();
        } catch (IOException e) {
            return false;
        }
    }

    private static String checksum(byte[] bytes) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
            StringBuilder hex = new StringBuilder();
            for (int i = 0; i < 8; i++) {
                hex.append(String.format("%02x", digest[i]));
            }
            return hex.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 is unavailable", e);
        }
    }
}
