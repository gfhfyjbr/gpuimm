package dev.gpuimm;

import android.content.Context;
import android.content.res.Configuration;
import android.graphics.SurfaceTexture;
import android.provider.Settings;
import android.util.LongSparseArray;
import android.view.Choreographer;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.TextureView;
import android.widget.FrameLayout;
import java.util.function.IntConsumer;
import java.util.function.Consumer;

/** UI-thread-only GPUI content. Add native controls above this view in the host. */
public final class GpuimmView extends FrameLayout implements TextureView.SurfaceTextureListener, Choreographer.FrameCallback {
    private final TextureView texture;
    public enum Backend { AUTO, VULKAN, OPEN_GL }
    private final Backend backend;
    private final LongSparseArray<Long> pointers = new LongSparseArray<>();
    private long nextTouch = 1;
    private long handle;
    private boolean active;
    private boolean attached;
    private boolean posted;
    private boolean disposed;
    public IntConsumer onSceneChanged;
    public Consumer<String> onFailure;

    public GpuimmView(Context context) {
        this(context, Backend.AUTO);
    }
    public GpuimmView(Context context, Backend backend) {
        super(context);
        this.backend = backend;
        texture = new TextureView(context);
        texture.setSurfaceTextureListener(this);
        texture.setOpaque(true);
        texture.setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
        texture.setOnTouchListener((view, event) -> sendTouches(event));
        addView(texture, new LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
    }

    private float scale() { return getResources().getDisplayMetrics().density; }

    @Override public void onSurfaceTextureAvailable(SurfaceTexture target, int width, int height) {
        // Let TextureView finish creating its hardware layer before configuring
        // the producer, and ignore a callback if its surface was already replaced.
        post(() -> {
            if (texture.isAvailable() && texture.getSurfaceTexture() == target) attachSurface(target);
        });
    }

    private void attachSurface(SurfaceTexture target) {
        if (disposed) return;
        Surface surface = new Surface(target);
        long created;
        try { created = GpuimmNative.attach(handle, surface, texture.getWidth(), texture.getHeight(), scale(), backend.ordinal()); }
        finally { surface.release(); } // Rust owns the acquired ANativeWindow reference.
        if (created == 0) { fail("Could not attach GPU surface"); return; }
        handle = created;
        attached = true;
        updateAppearance();
        setActive(active);
    }
    @Override public void onSurfaceTextureSizeChanged(SurfaceTexture target, int width, int height) {
        if (handle != 0) check(GpuimmNative.resize(handle, width, height, scale()));
    }
    @Override public boolean onSurfaceTextureDestroyed(SurfaceTexture target) {
        stopFrames();
        attached = false;
        pointers.clear();
        if (handle != 0) check(GpuimmNative.detach(handle));
        return true;
    }
    @Override public void onSurfaceTextureUpdated(SurfaceTexture target) {}

    public void setActive(boolean value) {
        active = value;
        if (handle != 0) check(GpuimmNative.lifecycle(handle, value && attached));
        if (!value) { pointers.clear(); stopFrames(); }
        else postFrame();
    }
    public void updateAppearance() {
        if (handle == 0) return;
        boolean dark = (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
        boolean reduced = Settings.Global.getFloat(getContext().getContentResolver(), Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0;
        check(GpuimmNative.appearance(handle, dark, reduced));
    }
    @Override protected void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        updateAppearance();
        if (handle != 0 && attached) check(GpuimmNative.resize(handle, texture.getWidth(), texture.getHeight(), scale()));
    }
    private void postFrame() {
        if (!posted && active && attached && !disposed) {
            posted = true;
            Choreographer.getInstance().postFrameCallback(this);
        }
    }
    private void stopFrames() {
        if (posted) Choreographer.getInstance().removeFrameCallback(this);
        posted = false;
    }
    @Override public void doFrame(long frameTimeNanos) {
        posted = false;
        if (!active || !attached || disposed || handle == 0) return;
        check(GpuimmNative.tick(handle));
        if (handle != 0 && onSceneChanged != null) onSceneChanged.accept(GpuimmNative.count(handle));
        postFrame();
    }
    public void advance() {
        if (handle != 0) {
            int value = GpuimmNative.advance(handle);
            check(value);
            if (value >= 0 && onSceneChanged != null) onSceneChanged.accept(value);
        }
    }
    public int sceneCount() { return handle == 0 ? -1 : GpuimmNative.count(handle); }
    public void setTitle(String title) { if (handle != 0) check(GpuimmNative.title(handle, title)); }
    public void memoryWarning() { if (handle != 0) check(GpuimmNative.memoryWarning(handle)); }
    public void dispose() {
        disposed = true;
        stopFrames();
        if (handle != 0) { GpuimmNative.destroy(handle); handle = 0; }
        pointers.clear();
    }

    private void check(int code) { if (code < 0) fail("GPU operation failed; see logcat"); }
    private void fail(String message) { dispose(); if (onFailure != null) onFailure.accept(message); }

    private boolean sendTouches(MotionEvent event) {
        if (handle == 0 || !active) return false;
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_CANCEL) {
            for (int i = 0; i < pointers.size(); i++) GpuimmNative.touch(handle, pointers.valueAt(i), 3, 0, 0);
            pointers.clear();
            return true;
        }
        if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN) {
            getParent().requestDisallowInterceptTouchEvent(true);
            int index = event.getActionIndex();
            pointers.put(event.getPointerId(index), nextTouch++);
            send(event, index, 0);
        } else if (action == MotionEvent.ACTION_MOVE) {
            for (int i = 0; i < event.getPointerCount(); i++) send(event, i, 1);
        } else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_POINTER_UP) {
            int index = event.getActionIndex();
            send(event, index, 2);
            pointers.remove(event.getPointerId(index));
            if (action == MotionEvent.ACTION_UP) getParent().requestDisallowInterceptTouchEvent(false);
        }
        return true;
    }
    private void send(MotionEvent event, int index, int phase) {
        Long id = pointers.get(event.getPointerId(index));
        if (id != null && handle != 0) check(GpuimmNative.touch(handle, id, phase, event.getX(index) / scale(), event.getY(index) / scale()));
    }
}
