package io.github.zacharyvmm.scah;

/** A call into scah failed. */
public final class ScahException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    private final ScahStatus status;

    ScahException(int code, String message) {
        super(message);
        this.status = ScahStatus.of(code);
    }

    /** Why the call failed. */
    public ScahStatus status() {
        return status;
    }
}
