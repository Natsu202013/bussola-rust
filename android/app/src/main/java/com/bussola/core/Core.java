package com.bussola.core;

/**
 * Thin UTF-free wrapper around the Rust core (libbussola.so).
 *
 * <p>The native side owns the filter state; this class keeps the opaque handle
 * in a {@code long} and forwards samples. Cardinal labels are mapped here so no
 * string ever crosses the JNI boundary.
 */
public final class Core implements AutoCloseable {

    /** Labels of the 16-point rose, matching {@code bussola-cardinal-index}. */
    private static final String[] CARDINALS = {
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
        "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"
    };

    private static final float DEFAULT_ALPHA = 0.18f;

    private long handle;

    public Core() {
        this(DEFAULT_ALPHA);
    }

    public Core(float alpha) {
        handle = nativeNew(alpha);
        if (handle == 0L) {
            throw new IllegalStateException("bussola: native init failed");
        }
    }

    private static void check(long h) {
        if (h == 0L) {
            throw new IllegalStateException("bussola: core already closed");
        }
    }

    /** Feeds a heading in degrees, returns the smoothed heading. */
    public float update(float degrees) {
        check(handle);
        return nativeUpdate(handle, degrees);
    }

    /** Feeds an {@code atan2}-style heading in radians. */
    public float updateRadians(float radians) {
        check(handle);
        return nativeUpdateRad(handle, radians);
    }

    public float heading() {
        check(handle);
        return nativeHeading(handle);
    }

    public void setOffset(float degrees) {
        check(handle);
        nativeSetOffset(handle, degrees);
    }

    public void reset() {
        check(handle);
        nativeReset(handle);
    }

    /** Cardinal label for an already smoothed heading. */
    public static String cardinal(float degrees) {
        int idx = nativeCardinalIndex(degrees);
        if (idx < 0 || idx >= CARDINALS.length) {
            return CARDINALS[0];
        }
        return CARDINALS[idx];
    }

    @Override
    public void close() {
        if (handle != 0L) {
            nativeFree(handle);
            handle = 0L;
        }
    }

    private static native long nativeNew(float alpha);
    private static native void nativeFree(long handle);
    private static native float nativeUpdate(long handle, float degrees);
    private static native float nativeUpdateRad(long handle, float radians);
    private static native float nativeHeading(long handle);
    private static native void nativeSetOffset(long handle, float degrees);
    private static native void nativeReset(long handle);
    private static native int nativeCardinalIndex(float degrees);

    static {
        System.loadLibrary("bussola");
    }
}
