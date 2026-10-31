package io.github.zacharyvmm.scah;

import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.LongConsumer;

/**
 * A native handle that is freed exactly once, never while a call is using it.
 *
 * <p>Every call takes the handle with {@link #acquire} and gives it back with {@link
 * #release}. {@link #run} (the owner's {@code close} or its cleaner) marks the handle closed,
 * so later {@code acquire}s throw, and frees it once no call is using it. This keeps a store
 * from being freed under a call on another thread, or by its cleaner while a method of the
 * store is still running.
 */
final class NativeResource implements Runnable {
    private static final int CLOSED = 1 << 30;

    private final long handle;
    private final LongConsumer free;
    private final String name;
    // The number of calls using the handle, plus CLOSED once closed.
    private final AtomicInteger state = new AtomicInteger();

    NativeResource(long handle, LongConsumer free, String name) {
        this.handle = handle;
        this.free = free;
        this.name = name;
    }

    /** Take the handle for a call. */
    long acquire() {
        while (true) {
            int current = state.get();
            if ((current & CLOSED) != 0) {
                throw new IllegalStateException("This " + name + " is closed");
            }
            if (state.compareAndSet(current, current + 1)) {
                return handle;
            }
        }
    }

    /** Give back a handle taken by {@link #acquire}. */
    void release() {
        if (state.decrementAndGet() == CLOSED) {
            free.accept(handle);
        }
    }

    /** Close the handle, freeing it now or when the last call releases it. */
    @Override
    public void run() {
        while (true) {
            int current = state.get();
            if ((current & CLOSED) != 0) {
                return;
            }
            if (state.compareAndSet(current, current | CLOSED)) {
                if (current == 0) {
                    free.accept(handle);
                }
                return;
            }
        }
    }
}
