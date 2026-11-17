package dev.gpuimm;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.graphics.Bitmap;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.EditText;
import java.util.HashSet;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.BooleanSupplier;

/** Runs against the real JNI library and checks the GPU texture, not only native UI. */
public final class MobileSmokeTest extends Instrumentation {
    private String backend;
    private Activity activity;
    private GpuimmView canvas;
    private TextureView texture;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        backend = arguments.getString("gpu-backend", "gl");
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            Intent intent = new Intent(getTargetContext(), DemoActivity.class)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK).putExtra("gpu-backend", backend);
            activity = startActivitySync(intent);
            runOnMainSync(() -> {
                canvas = find(activity.getWindow().getDecorView(), GpuimmView.class);
                texture = find(canvas, TextureView.class);
            });
            await("initial GPU frame", () -> canvas.sceneCount() == 0 && hasGpuPixels());
            runOnMainSync(() -> find(activity.getWindow().getDecorView(), Button.class).performClick());
            await("native action", () -> canvas.sceneCount() == 1 && hasGpuPixels());
            runOnMainSync(() -> find(activity.getWindow().getDecorView(), EditText.class).setText("From Android"));
            await("text update", this::hasGpuPixels);
            runOnMainSync(() -> canvas.setActive(false));
            runOnMainSync(() -> canvas.removeView(texture));
            waitForIdleSync();
            await("surface destroyed", () -> !texture.isAvailable() && canvas.sceneCount() == 1);
            runOnMainSync(() -> {
                canvas.addView(texture, new ViewGroup.LayoutParams(-1, -1));
                canvas.setActive(true);
            });
            await("surface restored with model retained", () -> texture.isAvailable() && canvas.sceneCount() == 1 && hasGpuPixels());
            runOnMainSync(() -> {
                canvas.memoryWarning();
                canvas.advance();
            });
            await("action after surface replacement", () -> canvas.sceneCount() == 2 && hasGpuPixels());
            runOnMainSync(() -> {
                activity.finish();
            });
            waitForIdleSync();
            result.putString("stream", "PASS: GPU pixels, native action/input, pause, detach/reattach, retained model, memory pressure, destroy\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "FAIL: " + error + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void await(String description, BooleanSupplier condition) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            AtomicBoolean ready = new AtomicBoolean();
            runOnMainSync(() -> ready.set(condition.getAsBoolean()));
            if (ready.get()) return;
            SystemClock.sleep(50);
        }
        throw new AssertionError("Timed out: " + description);
    }
    private boolean hasGpuPixels() {
        Bitmap bitmap = texture.getBitmap(64, 64);
        if (bitmap == null) return false;
        HashSet<Integer> colors = new HashSet<>();
        for (int y = 4; y < 60; y += 4) {
            for (int x = 4; x < 60; x += 4) colors.add(bitmap.getPixel(x, y));
        }
        bitmap.recycle();
        return colors.size() > 4;
    }
    private static <T extends View> T find(View view, Class<T> type) {
        if (type.isInstance(view)) return type.cast(view);
        if (view instanceof ViewGroup) {
            ViewGroup group = (ViewGroup) view;
            for (int i = 0; i < group.getChildCount(); i++) {
                T result = find(group.getChildAt(i), type);
                if (result != null) return result;
            }
        }
        return null;
    }
}
