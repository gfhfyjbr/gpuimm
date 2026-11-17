package dev.gpuimm;

import android.view.Surface;

final class GpuimmNative {
    static { System.loadLibrary("gpui_mobile_demo"); }
    static native long attach(long handle, Surface surface, int width, int height, float scale, int backend);
    static native int detach(long handle);
    static native int destroy(long handle);
    static native int tick(long handle);
    static native int resize(long handle, int width, int height, float scale);
    static native int lifecycle(long handle, boolean active);
    static native int appearance(long handle, boolean dark, boolean reducedMotion);
    static native int memoryWarning(long handle);
    static native int advance(long handle);
    static native int count(long handle);
    static native int title(long handle, String title);
    static native int touch(long handle, long id, int phase, float x, float y);
}
