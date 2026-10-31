package io.github.zacharyvmm.scah;

/** Why a call into scah failed. Mirrors {@code ScahStatus} in {@code scah.h}. */
public enum ScahStatus {
    /** A required argument was missing. */
    NULL_POINTER(1),
    /** An input string was not valid UTF-8. */
    INVALID_UTF8(2),
    /** A selector could not be compiled. */
    INVALID_SELECTOR(3),
    /** A parse was attempted without any queries. */
    EMPTY_QUERIES(4),
    /** The HTML nests elements deeper than the parser supports. */
    MAXIMUM_DEPTH_EXCEEDED(5),
    /** An element id or attribute index was out of range. */
    INDEX_OUT_OF_BOUNDS(6),
    /** scah hit an internal bug. Its state is intact. */
    INTERNAL_PANIC(7),
    /** The HTML, or the text saved from it, is 4 GiB or longer. */
    INPUT_TOO_LARGE(8),
    /** An argument was invalid. */
    INVALID_ARGUMENT(9),
    /** A result was too large to export. */
    EXPORT_TOO_LARGE(10),
    /** An input UTF-16 string had a lone surrogate, and the call rejects them. */
    INVALID_UTF16(11),
    /** A status newer than this version of the binding. */
    UNKNOWN(-1);

    private final int code;

    ScahStatus(int code) {
        this.code = code;
    }

    /** The status's value in the C ABI, or -1 for {@link #UNKNOWN}. */
    public int code() {
        return code;
    }

    static ScahStatus of(int code) {
        for (ScahStatus status : values()) {
            if (status.code == code) {
                return status;
            }
        }
        return UNKNOWN;
    }
}
