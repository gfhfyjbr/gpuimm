package dev.gpuimm;

import android.app.Activity;
import android.os.Bundle;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;

public final class DemoActivity extends Activity {
    private GpuimmView canvas;
    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setFitsSystemWindows(true);
        TextView heading = new TextView(this);
        heading.setText("gpuimm"); heading.setTextSize(32); heading.setPadding(dp(24), dp(12), dp(24), dp(8));
        root.addView(heading);
        EditText title = new EditText(this);
        title.setSingleLine(); title.setHint("Rename the GPU scene");
        title.setContentDescription("Scene title");
        LinearLayout.LayoutParams titleLayout = new LinearLayout.LayoutParams(-1, dp(56));
        titleLayout.setMargins(dp(24), 0, dp(24), dp(12));
        root.addView(title, titleLayout);
        FrameLayout stage = new FrameLayout(this);
        root.addView(stage, new LinearLayout.LayoutParams(-1, 0, 1));
        // The demo accepts an explicit driver override for emulator testing.
        GpuimmView.Backend backend = "gl".equals(getIntent().getStringExtra("gpu-backend"))
            ? GpuimmView.Backend.OPEN_GL : GpuimmView.Backend.AUTO;
        canvas = new GpuimmView(this, backend);
        stage.addView(canvas, new FrameLayout.LayoutParams(-1, -1));
        LinearLayout controls = new LinearLayout(this);
        controls.setGravity(Gravity.CENTER_VERTICAL);
        controls.setPadding(dp(20), dp(8), dp(12), dp(8));
        controls.setBackgroundColor(0xfff5f5f0);
        controls.setElevation(dp(8));
        TextView count = new TextView(this);
        count.setText("Scene 0"); count.setTextSize(18); count.setPadding(0, 0, dp(20), 0);
        count.setContentDescription("Scene count");
        controls.addView(count);
        Button next = new Button(this);
        next.setText("Next scene"); next.setOnClickListener(view -> canvas.advance());
        controls.addView(next, new LinearLayout.LayoutParams(-2, dp(56)));
        FrameLayout.LayoutParams overlay = new FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        overlay.bottomMargin = dp(24);
        stage.addView(controls, overlay);
        canvas.onSceneChanged = value -> count.setText("Scene " + value);
        canvas.onFailure = count::setText;
        title.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
            public void onTextChanged(CharSequence s, int start, int before, int count) { canvas.setTitle(s.toString()); }
            public void afterTextChanged(Editable value) {}
        });
        setContentView(root);
    }
    @Override public void onResume() { super.onResume(); canvas.setActive(true); }
    @Override public void onPause() { canvas.setActive(false); super.onPause(); }
    @Override public void onTrimMemory(int level) { super.onTrimMemory(level); if (canvas != null) canvas.memoryWarning(); }
    @Override public void onDestroy() { canvas.dispose(); super.onDestroy(); }
}
