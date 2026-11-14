# Native Glimm transitions

This local fork ports the shader effects shown at [glimm.dev/#demos](https://glimm.dev/#demos) into GPUI's Metal, WGPU and Direct3D 11 renderers. The source is the MIT-licensed `glimm@0.4.1` package. Its copyright notice is in [GLIMM-LICENSE](GLIMM-LICENSE).

The implementation uses native GPUI scenes and renderer-owned GPU textures. It does not embed a browser, capture the desktop, or require HTML-in-Canvas support. The playground reproduces the nine original examples using measured layouts, original content and decorative resources; text and controls remain native. Browser and native rasterization are not pixel-identical.

## Run

From this repository:

```sh
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
  cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support
```

`SDKROOT` is macOS-only: it keeps the linker and SDK from the same Xcode installation. Nothing changes the system's selected Xcode or global environment. On Linux and Windows, run the same Cargo command without `SDKROOT`. Linux needs the normal GPUI X11/Wayland, font and Vulkan dependencies; Windows needs Rust and the Windows SDK.

The repository root also provides `./run-glimm.sh` for macOS/Linux and `.\run-glimm.ps1` for Windows PowerShell. Both select the same native example and enable its capture support.

The playground includes the nine original scenes: Generate data, Teammate joins, Invoice paid, Upgrade to Pro, Usage reset, Publish, Autopilot, Theme and Tasks. Original controls run their corresponding state transitions, work delays, shapes and palettes. The external customization panel changes settings for the next action.

- **Run original action** starts the selected example, including its original work delay when present (Sheet 2600ms, Upgrade 500ms, Publish 1400ms).
- **Replay scene** restores the initial scene and its automatic behavior. Join starts after 1200ms, Invoice after 900ms, and Usage reset counts down from three seconds. Tasks waits for checkbox input.
- **Cancel** rejects pending work and lets the gathered content settle. The Sheet's own Stop button does the same.
- **Pause / Resume** freezes the current effect instant without resetting the sweep.
- **Fast forward** completes traversal in 160ms.
- **Copy JSON** exports every option, including ones without a dedicated slider.
- **Reduced motion** skips the visual effect without losing successful work results.
- **Show point of action** marks the effective scope-local origin. **Origin: original element / custom options** selects native element bounds or configured coordinates.

For a frozen gather capture, use `--gather --time 1600 --capture frame.png`; there is no separate gather button in the native panel.

Use Tab, Enter/Space and Escape to operate the controls. Buttons expose accessible names and keyboard focus outlines.

## Wrap content

Keep one controller per mounted scope. Clone that controller into event handlers; do not attach the same controller to multiple scope elements.

```rust
use gpui::{GlimmController, GlimmDemo, GlimmOptions, glimm_scope};

// Store on the view, not inside render():
let controller = GlimmController::new();

// In Render, wrap the native element tree:
let content = glimm_scope("page-transition", &controller, page_content);

// In an event handler, capture BEFORE changing application state:
controller.play(GlimmDemo::Generate.options(), window, cx);
self.show_generated_data = true;
cx.notify();
```

The wrapper forwards layout, prepaint, input, focus and accessibility to its child. It does not intercept pointer events. Use the child's normal GPUI overflow/clipping controls when it should be a clipped viewport.

Scope content is rasterized separately and put back at its original paint position. Elements painted above it remain above it. Nested scopes rasterize from the inside out. Ancestor content masks and rounded-clip operations survive scene extraction and replacement. An idle scope schedules no animation frames and holds no effect textures in the renderer.

## Asynchronous work

```rust
let token = controller.anticipate(options, window, cx);

// Once work completes, inside a window/view update:
if controller.resolve(token, window, cx) {
    self.data = completed_data;
    cx.notify();
}

// On failure or explicit cancellation:
controller.cancel(token, window, cx);
```

`resolve` accepts only the matching pending token, at most once. A newer run or cancellation makes earlier results stale. It captures the old scene immediately before the caller commits the new data. Reduced motion still accepts the matching result, but allocates no effect targets.

`stop(window)` removes the visual effect immediately and invalidates pending work. `fast_forward(window, cx)` completes traversal in 160ms. `pause`, `seek(elapsed_ms, window)` and `resume` support inspection and reproducible captures. `take_midpoint(cx)` delivers one midpoint event per run: the configured threshold for a band, or the 90ms burst delay for a scope. Call it from a frame-driven view if application work depends on that event.

Starting another `play` replaces the current run and snapshots the current native scene. It is not a queue: it does not preserve the previous shader's already-deformed framebuffer. Applications that require that particular interruption policy should finish/cancel the current run before committing another state.

## Ordering and origin helpers

`play` must capture the old frame before the application changes its state, and `resolve` must come before the result is committed. Two wrappers make the wrong order impossible to write:

```rust
// Clone the controller out first: the closure borrows the view.
let controller = self.controller.clone();
controller.transition(options, window, cx, |_, cx| {
    self.show_generated_data = true;
    cx.notify();
});

// Returns None, without running the closure, for a stale or cancelled token.
controller.resolve_with(token, window, cx, |_, cx| {
    self.data = completed_data;
    cx.notify();
});
```

`glimm_origin(&controller, element)` wraps the element an effect starts from. It changes neither layout nor painting; it records where the element was laid out, and `controller.marked_point()` returns its centre in scope coordinates, ready for `options.at`. The marker may be inside or outside the scope.

## Radial distortions

Besides glimm.dev's two paths, `GlimmEffect` has three effects of this fork that bend the scope's content around a point of action instead of sweeping across it. The point is `options.at` in scope pixels (or `origin` and `origin_offset`), and it may lie outside the scope: a front then starts just short of the scope's nearest edge, so no part of the run is spent where nothing shows.

| Effect | Preset | What it does |
| --- | --- | --- |
| `Emerge` | `GlimmOptions::emerge()` | New content comes out of the point through a membrane. Inside a growing bubble it is squeezed toward the point (the content out to the scope's edge is pulled in, most at the rim) and blurred as much as it is squeezed; it relaxes as the front passes, overshoots and settles. At the rim it refracts and splits into colours. The old content ahead of the rim is pushed aside and bent around it. |
| `Shock` | `GlimmOptions::shock()` | No change of content: a raised ring of refraction runs out from the point, ripples trail it and a weaker echo ring follows at six tenths of its radius, all weakening with distance. It draws what the scope holds now, never the snapshot. |
| `Collapse` | `GlimmOptions::collapse()` | The reverse of Emerge: a front closes in on the point, the old content inside it is squeezed into the point and gone, and what replaces it is pulled in after it. |

**Hold.** `anticipate` with any of the three holds a breathing lens around the point while the work runs: content is drawn toward the point, deeper on each in-breath, with a ripple leaving the point once a breath. `resolve` releases it straight into the effect (the lens lets go over 260ms while the front starts); `cancel` lets it relax over 320ms. While it holds, the run has no end (`end_ms()` is `None`), as for glimm.dev's gather.

The palette lives in the refracted light, not over it: where the front bends content, its colours separate along the ray, weighted from red to blue or, as `tint` rises, through the palette; and the front's leading edge carries a thin rim of the palette that brightens where it refracts bright content. Both scale with the wave's energy, which is spent by the end, so the last frame is the content exactly. The iridescent band remains available as an optional layer, drawn as a ring on the front: `band_overlay`.

A node that crawls out of its parent while the canvas around it ripples uses two scopes, the canvas's and the node's own, nested in it:

```rust
// On the canvas view: one controller for the board, one per node, kept across frames.
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    glimm_scope("board", &self.board_fx, div().size_full().children(self.nodes.iter().map(|node| {
        // The node's scope is the node plus a margin the wave can use.
        glimm_scope(("node", node.id), &node.fx, node_view(node).p(px(48.)))
    })))
}

// The parent's agent starts creating a node: the board holds around the parent's edge.
let edge = parent_bounds.right_center(); // window coordinates
self.pending = Some(self.board_fx.anticipate(
    GlimmOptions { at: Some(self.board_fx.local_point(edge)), ..GlimmOptions::shock() },
    window,
    cx,
));

// The node exists: release the hold into the ripple and pull the node out of the edge.
if let Some(token) = self.pending.take()
    && self.board_fx.resolve(token, window, cx)
{
    let fx = GlimmController::new();
    // The node's scope has not been laid out yet, so the point comes from where it will be.
    // It is outside the node's scope, on the parent's edge.
    let at = [f32::from(edge.x - node_origin.x), f32::from(edge.y - node_origin.y)];
    fx.play(GlimmOptions { at: Some(at), ..GlimmOptions::emerge() }, window, cx);
    self.nodes.push(Node { id, fx, .. }); // commit after play: its snapshot is the empty scope
    cx.notify();
}
```

`play` on a scope that has never been painted captures nothing, which is what an emerging node's old content is. For a node that leaves, keep its scope mounted, with the node still laid out and its content replaced by nothing, until `end_ms()` has passed, and play `GlimmOptions::collapse()` on it.

| Field | Range | Emerge / Shock / Collapse | Meaning |
| --- | --- | --- | --- |
| `amplitude` | 0..96 px | 18 / 14 / 18 | How far the front's refraction moves content, times `intensity`. The ring widens to at least 3.3 × amplitude so it never folds content over itself. |
| `ringWidth` | 4..600 px | 44 / 36 / 44 | Width of the wavefront ring; also the reach of Emerge's push and the old/new seam's width. |
| `wavelength` | 8..600 px | 60 / 48 / 60 | Spacing of the ripples trailing the front; their height is capped at a tenth of it. |
| `ripples` | 0..6 | 2.5 / 1.2 / 2.5 | Length of the trailing ripple train, in wavelengths. |
| `echo` | 0..1 | 0 / 0.45 / 0 | Shock only: strength of the second ring at six tenths of the first's radius. |
| `falloff` | 16..100000 px | 900 / 1000 / 900 | Distance from the point at which the wave has half its strength. |
| `chroma` | 0..2 | 0.6 | How far the refracted colours separate, relative to the refraction. |
| `tint` | 0..1 | 0.45 | How much the palette colours the separated light and the rim; 0 is uncoloured refraction. |
| `stretchBlur` | 0..2 | 1 | Blur per unit of stretch: squeezed content softens through the mipmaps, stretched content smears along the ray. |
| `squeeze` | 0..1 | 0.7 / – / 0.85 | Emerge and Collapse: share of the content's distance to the scope's edge pulled into the bubble. |
| `push` | 0..0.9 | 0.4 / – / 0.3 | Emerge pushes the content outside the front aside, Collapse pulls it in after it. |
| `holdAmount` | 0..2 | 1 | Strength of the breathing lens while `anticipate` waits. |
| `holdRadius` | 8..2000 px | 90 | Radius of the breathing lens. |
| `breathMs` | 200..10000 | 1400 | One breath. |
| `bandOverlay` | 0..1.5 | 0 | Opacity of the optional iridescent band ring on the front. |

The radial effects also use `sweepMs` (the run's length: 1150 / 1200 / 950), `easing` (the front's travel; the presets use gentle cubic Béziers), `gatherMs` (how long the hold takes to build), `intensity` (scales amplitude, squeeze, push and hold within their limits), `palette`, `hueShift`, `brightness` and `feather` (displacement fades out over it at the scope's edges, at least 6px, so the edge's pixels are never repeated). The glimm.dev sweep settings (`direction`, `curve`, `span`, `bandTight`, `fade`, `peakAlpha`, wave and ripple amounts, `swellAmount`, `pageEffect`) do not apply. The timelines are pure functions of elapsed time like the others: `seek` reproduces any instant, `fast_forward` finishes in 160ms, `take_midpoint` fires 90ms after the front starts.

**Cost.** A pixel samples the scope's textures once where nothing bends, and five times along the ray where the content is refracted or stretched by more than a third of a pixel (dispersion and smear share those taps; squeeze blur comes from the mipmaps at no extra taps). On the seam between old and new content of Emerge and Collapse, a band about 0.6 ring widths wide, both layers are sampled: ten taps, the worst case. glimm.dev's scope takes up to 36 there (16-tap blur on each layer plus the colour split). The rest is arithmetic: a few exponentials and a sine per evaluation of the wave, evaluated twice per layer to measure the stretch.

**Playground.** `--distort emerge|shock|collapse|hold|spawn` opens a stage with a grid, text and two cards in place of the nine examples: `spawn` is the two-scope setup above, `hold` breathes around a busy node and releases into a shock, `--full` runs Emerge or Collapse on the whole stage. With `--capture file.png` it renders in a hidden window: `--time ms` for one instant, `--strip 8` for eight instants across the run tiled into one image, `--debug-view` and `--config` as for the examples, `--width`/`--height` for the stage. `make dev ARGS="--distort spawn"` follows the shader file while the stage plays.

## Development loop

These aids are for building new effects; none of them changes a frame unless asked to.

**Follow the shader file.** Set `GPUI_GLIMM_SHADER_DIR` to a directory holding `glimm.metal`, `glimm.wgsl` or `glimm.hlsl`. The renderer compiles that file in place of its built-in source and compiles it again whenever the file's modification time changes. `./run-glimm.sh --dev` (`.\run-glimm.ps1 -dev`) points it at the platform's own source file, so saving the shader in an editor updates the running playground. A file that fails to build leaves the previous shader in use; before any shader has built, the built-in one is used. The compiler's message, with line and column, is shown under the preview, printed to stderr, and available from `gpui::glimm_shader_error()`. An application that wants the same loop polls `gpui::glimm_shader_stamp()` and calls `window.refresh()` when it changes, which the playground does every 200ms while something is followed; with nothing followed it does not poll.

**Look inside the effect.** `controller.set_debug_view(view, window)` replaces the transition with a false-colour view of the content shader, from the next frame, a paused one included:

| View | Shows |
| --- | --- |
| `Mix` | which frame each pixel samples: old black, new white |
| `Displacement` | the sampling offset: red horizontal, green vertical, mid-grey none (±32 logical pixels full scale) |
| `Blur` | blur radius, black sharp, white 32 logical pixels |
| `Front` | crest red, trough green, cross-axis span mask blue |

The value travels in the fourth component of uniform row 9, which the palette does not use; the same branch exists in all three shaders. The playground has a button for each view, and `--debug-view mix|displacement|blur|front` selects one for a capture.

**Scrub time.** The bar under the preview seeks the run and pauses it: click or drag, Left/Right to step one 60Hz frame, Shift for 100ms. It spans `controller.end_ms()`, which is unknown while a run still gathers for unfinished work; then it spans what has gathered so far. `controller.elapsed_ms(cx)` reads the current position. Seeking does not undo what the application did at the burst, so scrubbing back across it shows the shader's earlier frame over the later application state.

**Tune settings live.** Edits to the `--config` file apply while the window is open: its keys overlay the panel's current settings, and `controller.retune(options, window)` hands them to a run in flight without capturing a new frame, so a paused frame redraws with the new values. A file that does not parse is reported and ignored.

## Original examples and element-relative scopes

The playground reproduces the nine original embedded examples, not a generic application with nine parameter presets. Its reference frame is 634 × 476 logical pixels. Layout measurements in `crates/gpui_perf/src/fast/glimm_assets/scenes.json` become native GPUI quads, shaped Inter text, images and accessible hit targets. PNGs contain original photographs, SVG/CSS illustrations, gradients and decorative backdrop textures, never a flattened page screenshot. No browser or website code runs in the application.

The Sheet scope is **only** `(0,116,634,360)`. Its three top panels and floating composer are siblings. The action point is the composer's center relative to the table viewport, `[317,293]`. The invoice banner, blue glow and dots are also outside its scope. Other examples use their original full embedded viewport with the original anchor, curve, span, intensity and fade; a full-sized scope is not a whole-native-window effect.

Convert actual element geometry into scope coordinates:

```rust
let center_in_window = element_bounds.center();
options.at = Some(controller.local_point(center_in_window));
let token = controller.anticipate(options, window, cx);
```

Record native element bounds during prepaint. `controller.bounds()` and `local_point()` use logical window coordinates; moving either element does not require renormalizing against the whole window. The examples record the real stage bounds and use the source-measured native element rectangles inside that stage.

For the reference's `anticipate: true` **without** a work promise, call `anticipate_change` and resolve after `gather_ms`. It gathers to the full pull of 1; work anticipation through `anticipate` deliberately holds at 0.32. For custom entrance animations before a sweep, `prepare(window)` preserves the old scoped frame for the next start and resolve, exposing the package's `prepare()` / `fromOld` path. The site's teammate demo instead changes its avatars on burst and opens its popover on leave.

`options.full_bleed: Some(true)` treats an embedded full-page scene as viewport-sized even when the native window has external controls. `None` compares against the native window. The Sheet preset explicitly uses `Some(false)`.

## Settings

`GlimmOptions` supports serde JSON with camelCase field names and defaults for omitted fields. `validated()` replaces non-finite inputs and clamps ranges before uniforms reach the GPU; controllers call it automatically.

| Field | Scope default | Meaning |
| --- | --- | --- |
| `effect` | `"Scope"` | Content deformation; `"Band"` is overlay only |
| `palette` | spectrum coefficients | Cosine palette `{a,b,c,d}`, three channels per coefficient |
| `direction` | `"ttb"` | `ltr`, `rtl`, `ttb`, `btt` |
| `easing` | `"ease"` | Band easing; scope uses the reference's power-2.4 traversal |
| `sweepMs` | 1600 | Traversal duration |
| `outroMs` | 220 | Band alpha outro |
| `midpoint` | 0.56 | Band callback threshold |
| `gatherMs` | 620 | Gather duration while anticipating work |
| `peakAlpha` | 1.1 | Band opacity multiplier |
| `brightness` | 1.15 | Palette brightness |
| `bandTight` | 28 | Gaussian tightness |
| `waveAmount` | 0 | Slow edge displacement |
| `rippleAmount` | 1 | Foil colour ripple |
| `waveSpeed` | 1 | Internal wave clock multiplier |
| `swellAmount` | 0.15 | Specular and Fresnel contribution |
| `curve` | 0 | Parabolic cross-axis lag |
| `span` | 1 | Cross-axis coverage |
| `origin` | `[0.5,0]` | Normalized top-down point of action |
| `originOffset` | `[0,-12]` | Offset in logical pixels |
| `at` | null | Explicit scope-local logical-pixel point; overrides origin |
| `intensity` | 1 | Content displacement/blur multiplier |
| `fade` | null | Optional progress interval `[from,to]` |
| `feather` | null | Automatic edge feather, or explicit logical pixels |
| `fullBleed` | null | Optional viewport-sized treatment for an embedded native scene |
| `pageEffect` | true | Disable displacement/blur while keeping the colour band |
| `hueShift` | 0 | Explicit deterministic palette phase in 0..0.4 |

`GlimmOptions::band()` selects the provider/overlay defaults rather than scope defaults. `GlimmPaletteName::ALL` resolves the seven shipped presets. `GlimmPalette::from_colors(&[0xRRGGBB, ...])` uses the original shortest-arc OKLCH interpolation and least-squares cosine fit. Named easings and custom `CubicBezier([x1,y1,x2,y2])` are supported.

An example JSON file can contain only overrides:

```json
{
  "direction": "btt",
  "sweepMs": 2200,
  "gatherMs": 800,
  "curve": 0.2,
  "span": 0.72,
  "intensity": 1.3,
  "origin": [0.5, 1],
  "originOffset": [0, -12]
}
```

Pass it with `--config path/to/settings.json`.

## Capture

Screenshot support needs the `gpui_platform_fast/test-support` feature used by the launch command above.

```sh
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
  cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support -- \
  --capture glimm-frame.png --time 280 --demo 0
```

`--demo 0..8` selects an original scene. `--scene-only` captures just its 634 × 476 reference viewport; otherwise the native window also shows the external customization panel. `--state initial|busy|final|transition` selects a deterministic state. `--time` is elapsed time since the native action, including its work delay. `--gather` leaves work unresolved. `--width` and `--height` set the external window size (minimum 1000 × 720). Captures are PNGs from the actual platform renderer (Metal, WGPU or Direct3D 11), without screen-recording permission; the application exits after saving. Linux X11 and Wayland use the window's own WGPU device and sprite atlas, not a second renderer.

`smoke-glimm.mjs` writes 31 before/after/transition, gather, Band, custom-option and full-window captures for all nine examples. `GLIMM_DEMO_BIN`, `GLIMM_CAPTURE_PREFIX` and `GLIMM_SMOKE_RESULTS` select a platform executable and separate output names. The repository's `compare-glimm-platforms.py linux|windows` builds reference/native comparisons and checks Sheet isolation for those captures. `compare-glimm-scenes.py` compares them to actual embedded browser references and requires zero changed pixels in the Sheet's three top panels, composer prompt and Add button during gather. Browser/CoreText antialiasing, corner shapes and shadow kernels are not bit-identical; these layout comparisons are separate from the shader fixture below.

## Renderer and verification

Code lives in:

- `crates/gpui/src/fast/glimm/`: options, palette fitting, pure timeline, scope, controller and scene metadata; `dev.rs` holds the shader-following source, build status and debug views.
- `crates/gpui_apple/src/fast/glimm.rs`: isolated-scene compositor and temporary texture ownership.
- `crates/gpui_apple/src/fast/glimm.metal`: original band, suction, progressive mipmapped blur, travelling crest/trough, old/new dissolve, chromatic split and dimming formulas.
- `crates/gpui_wgpu/src/fast/glimm.rs` and `glimm.wgsl`: WGPU compositor, premultiplied scope raster, cropped mip passes and cached textures.
- `crates/gpui_windows/src/fast/glimm.rs` and `glimm.hlsl`: Direct3D 11 compositor, runtime shader compilation, cropped mip generation and device-recovery reset.
- `crates/gpui/src/fast/glimm/gpu_tests.rs`: shared compositor regressions and original-WebGL fixture, used by all three backend test adapters.
- `crates/gpui_perf/src/fast/glimm_demo.rs`: original example interactions, anchors and external customization controls.
- `crates/gpui_perf/src/fast/glimm_scenes.rs`: source-measured native primitives, Inter text and original decorative resources.
- `crates/gpui_perf/src/fast/glimm_motion.rs`: source spring samples and timing for the Invoice entrance, Join popover and joined avatars.
- `crates/gpui_perf/examples/glimm.rs`: thin executable entry point.

The shader ports account for opposite framebuffer Y directions and premultiplied alpha. Scope rasters clear to transparent, then normalize to the straight-alpha format expected by GPUI's sprite pipeline. Mipmaps are generated from scope-cropped textures, not the surrounding window, to prevent edge colour/alpha bleed. The 13-row uniform format has an explicit C layout. Scene range replay preserves scope identity across retained views. Async commits advance the capture id, so a previously rasterized gather frame cannot hide a fresh old-state snapshot.

```sh
# macOS
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
  cargo test -p gpui -p gpui_apple --features gpui_apple/test-support glimm --lib
# Linux (Vulkan)
cargo test -p gpui_wgpu --features test-support --lib fast::glimm -- --nocapture
# Windows (explicit software D3D11 WARP adapter)
cargo test -p gpui_windows --features test-support --lib fast::glimm -- --nocapture
# Native scene/interaction tests on each OS
cargo test -p gpui_perf --example glimm --features gpui_platform_fast/test-support
script/check-upstream
```

Core fixtures evaluate the published JavaScript palettes and easing functions. GPU tests exercise actual content displacement, four-way old/new compositing, clipping, paint-order overlays, nested scopes, transparent content, cropped mipmap edges, texture release at idle, and a golden image rendered by the original WebGL shader.

The workspace's `verify-glimm.mjs` runs that original GLSL in headless Chrome against the same textures and uniforms as the shared backend tests. For the checked 512 × 320 combined deformation/band fixture:

| Runtime | Mean absolute RGB error / 255 | Maximum channel error / 255 | Glimm GPU tests |
| --- | ---: | ---: | ---: |
| macOS / Metal | 0.152102 | 2 | 12 passed |
| macOS / WGPU-Metal | 0.152100 | 2 | 14 passed |
| Ubuntu ARM64 Tart / Vulkan-lavapipe | 0.209757 | 2 | 13 passed (before the shader-following test was added; not rerun) |
| Windows 11 ARM64 Tart / D3D11 WARP | 0.162974 | 2 | 13 passed |

Windows checks execute a cross-compiled x86_64 GNU executable under Windows ARM64's x64 emulation, not a native ARM64 binary. The WARP harness explicitly chooses the software adapter. Linux native-window captures use X11/Xvfb; the Wayland capture hook is compiled but has not been separately exercised in a Wayland compositor.

These are measured fixtures, not a claim that every frame, font rasterizer or GPU backend is bit-identical. The original WebGL path has an additional 8-bit intermediate write; GPU mipmap/filter rounding also varies. The full Linux WGPU library suite has four existing path-tile rasterization failures on lavapipe, reproduced unchanged in a clean checkout of the base commit. All Glimm-specific tests pass; those unrelated failures are not hidden or relabeled as passes.

## Limits

- The effect has Metal (macOS), WGPU (Linux) and Direct3D 11 (Windows) implementations. Linux Tart verification uses Vulkan/lavapipe; Windows Tart has no GPU acceleration and uses Direct3D WARP. These software-adapter checks do not establish hardware GPU performance. FreeBSD, browser and mobile targets have not been runtime-verified.
- Ancillary Invoice and Join entrances use the source spring and timing. Their transient CSS blur is not reproduced exactly; avatar ring dasharray interpolation uses a crossfade between original ring states. Native font antialiasing, corner geometry and shadow kernels can also differ from Chrome.
- Coverage is the band and content effects used by the nine demos at the supplied URL. React/Next adapters, DOM APIs, the package's separate experimental 3D mesh/NameDrop renderers and its component library are not part of this native port.
- GPU-backed content already painted into a GPUI scene can be sampled. Separate OS-native child views, such as a platform WebView/video surface outside the scene, cannot be deformed by this shader.
- Retain one controller per scope, and capture before committing state. Arbitrary application-specific transition queues are not implemented.
- Active scopes use full-viewport raster scratch space plus scope-cropped old/current textures and per-run output textures. These are reused during animation and released when idle; many simultaneously active scopes can still use substantial GPU memory. The per-run output is viewport-sized whatever the scope's size, so a small scope in a large window costs a whole window's worth of pixels while it runs.
