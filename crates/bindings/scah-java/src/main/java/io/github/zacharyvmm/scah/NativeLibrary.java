package io.github.zacharyvmm.scah;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.PosixFileAttributes;
import java.nio.file.attribute.PosixFilePermission;
import java.nio.file.attribute.PosixFilePermissions;
import java.nio.file.attribute.UserPrincipal;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.Locale;
import java.util.Set;
import java.util.stream.Stream;

/**
 * Finds a native library on disk.
 *
 * <p>When the {@code scah.library.dir} system property names a directory, the library is
 * loaded from there. Otherwise it is extracted from the jar, under {@code
 * io/github/zacharyvmm/scah/native/<rid>/}, into a directory named after its checksum in a
 * cache only this user can write (see {@link #cacheRoot}). An existing copy is used only when
 * its bytes match the jar's, and extraction is atomic, so concurrent JVMs share one copy.
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

        try {
            Cache cache = cacheRoot(cacheBase());
            Path directory = cache.root.resolve(checksum(library));
            Path target = extract(library, directory.resolve(rid).resolve(file));
            if (cache.temporary) {
                // Files delete on exit in the reverse order they were registered.
                for (Path path : new Path[] {cache.root, directory, target.getParent(), target}) {
                    path.toFile().deleteOnExit();
                }
            }
            return target;
        } catch (IOException e) {
            throw new UncheckedIOException("Cannot extract " + resource, e);
        }
    }

    /** The directory caches go in: {@code scah.library.cache}, or {@code java.io.tmpdir}. */
    static Path cacheBase() {
        return Paths.get(System.getProperty("scah.library.cache", System.getProperty("java.io.tmpdir")));
    }

    /** A new directory in the private cache, for this JVM alone. */
    static Path privateDirectory(String prefix) throws IOException {
        return Files.createTempDirectory(cacheRoot(cacheBase()).root, prefix);
    }

    /** Write {@code library} to {@code target} unless an identical copy is already there. */
    private static Path extract(byte[] library, Path target) throws IOException {
        if (holds(target, library)) {
            return target;
        }
        Files.createDirectories(target.getParent());
        Path temporary = Files.createTempFile(target.getParent(), target.getFileName().toString(), ".tmp");
        try {
            Files.write(temporary, library);
            try {
                Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException e) {
            // Another JVM may have extracted it first and, on Windows, hold it open.
            if (!holds(target, library)) {
                throw e;
            }
        } finally {
            Files.deleteIfExists(temporary);
        }
        return target;
    }

    private static boolean holds(Path path, byte[] contents) throws IOException {
        return Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)
                && Files.size(path) == contents.length
                && Arrays.equals(Files.readAllBytes(path), contents);
    }

    /**
     * A cache directory that only this user can write, so no one else can plant a library in
     * it: {@code scah-<user>} under {@code base}, which is {@code scah.library.cache} or {@code
     * java.io.tmpdir}.
     *
     * <p>When another user owns that path, or others can write to it, a new private temporary
     * directory is used instead, and deleted when the JVM exits. Windows' temporary directory is
     * private to each user already.
     */
    static Cache cacheRoot(Path base) throws IOException {
        base = base.toAbsolutePath();
        if (!FileSystems.getDefault().supportedFileAttributeViews().contains("posix")) {
            return new Cache(Files.createDirectories(base.resolve("scah")), false);
        }
        Files.createDirectories(base);
        String user = System.getProperty("user.name", "").replaceAll("[^A-Za-z0-9._-]", "_");
        Path shared = base.resolve("scah-" + user);
        try {
            Files.createDirectory(shared, PosixFilePermissions.asFileAttribute(OWNER_ONLY));
        } catch (FileAlreadyExistsException e) {
            // Checked below.
        }
        if (isPrivate(shared, base)) {
            return new Cache(shared, false);
        }
        return new Cache(Files.createTempDirectory(base, "scah-"), true);
    }

    /** A cache directory, and whether it is this JVM's own, to delete on exit. */
    static final class Cache {
        final Path root;
        final boolean temporary;

        Cache(Path root, boolean temporary) {
            this.root = root;
            this.temporary = temporary;
        }
    }

    private static final Set<PosixFilePermission> OWNER_ONLY = PosixFilePermissions.fromString("rwx------");

    /** Whether {@code dir} is a real directory that this user owns and only this user can write. */
    private static boolean isPrivate(Path dir, Path base) throws IOException {
        PosixFileAttributes attributes;
        try {
            attributes = Files.readAttributes(dir, PosixFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        } catch (NoSuchFileException e) {
            return false;
        }
        Set<PosixFilePermission> permissions = attributes.permissions();
        return attributes.isDirectory()
                && attributes.owner().equals(currentUser(base))
                && !permissions.contains(PosixFilePermission.GROUP_WRITE)
                && !permissions.contains(PosixFilePermission.OTHERS_WRITE);
    }

    /** The owner of files this process creates, which also works for users without a name. */
    private static UserPrincipal currentUser(Path base) throws IOException {
        Path probe = Files.createTempFile(base, "scah-", ".owner");
        try {
            return Files.getOwner(probe);
        } finally {
            Files.delete(probe);
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

    /**
     * Whether this JVM runs on musl. Asks which C library the process mapped, since a glibc system
     * may have musl's loader installed too.
     */
    static boolean isMusl() {
        try (Stream<String> maps = Files.lines(Paths.get("/proc/self/maps"))) {
            return maps.anyMatch(line -> line.contains("/ld-musl-") || line.contains("/libc.musl-"));
        } catch (IOException | UncheckedIOException e) {
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
