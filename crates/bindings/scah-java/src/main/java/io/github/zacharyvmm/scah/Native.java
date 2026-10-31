package io.github.zacharyvmm.scah;

/** The backend, loaded on first use. */
final class Native {
    private static final Backend BACKEND;
    private static final RuntimeException RUNTIME_FAILURE;
    private static final Error ERROR_FAILURE;

    static {
        Backend backend = null;
        RuntimeException runtimeFailure = null;
        Error errorFailure = null;
        try {
            backend = Backends.load();
        } catch (RuntimeException e) {
            runtimeFailure = e;
        } catch (LinkageError e) {
            errorFailure = e;
        }
        BACKEND = backend;
        RUNTIME_FAILURE = runtimeFailure;
        ERROR_FAILURE = errorFailure;
    }

    private Native() {}

    /** The backend, or the reason it failed to load, every time it is asked for. */
    static Backend backend() {
        if (RUNTIME_FAILURE != null) {
            throw RUNTIME_FAILURE;
        }
        if (ERROR_FAILURE != null) {
            throw ERROR_FAILURE;
        }
        return BACKEND;
    }
}
