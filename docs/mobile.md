# Native mobile embedding

`gpui_mobile` embeds a GPUI/Glimm scene in an existing iOS or Android application.
The native application owns the event loop, navigation, keyboard and controls;
Rust owns the GPU scene and its model. `gpui_mobile_demo` contains working host
views, a small C/JNI bridge and runnable applications.

## What the codebase already provided

- `gpui::Application::run_embedded` retains an application without taking over
  the host event loop. `Platform` and `PlatformWindow` are the integration seams.
- `gpui_ios` already supplies a standalone UIKit application backend, with
  CoreText and iOS input support. It is useful when GPUI owns the application.
  This new backend instead lets an existing UIKit/SwiftUI application own it.
- `gpui_wgpu` supplies the renderer, glyph atlas, Glimm shaders and surface
  replacement. The mobile adapter reuses these instead of translating GPUI
  elements to native widgets. Its native instance now enables Metal on iOS.
- The existing native window composition implementation targets desktop
  AppKit, Win32 and Linux surfaces. It does not supply Android views or a
  mobile native-element tree. There was no Android platform backend.

The additions are in two independent crates. The only changes inside an
upstream renderer file are calls to `fast::mobile` for backend selection and
Android debug-label handling; no GPUI public API is changed.

## Composition and native Liquid Glass

```text
UIKit / SwiftUI                     Android Views / Compose
  UIView + CAMetalLayer               TextureView + Surface
            │                                 │
            └──────── gpui_mobile ──────────────┘
                      GPUI → WGPU
                      Metal / Vulkan / GLES

Native sibling controls are composed above the GPU view by the operating system.
```

The iOS demo places `UIVisualEffectView(effect: UIGlassEffect(style: .regular))`
and a `UIButton.Configuration.prominentGlass()` button above the live Metal
content on iOS 26+. Earlier systems use a native blur and filled button. These
are real UIKit controls, with native focus, accessibility and hit testing.
The Android example uses an ordinary `EditText` and `Button` above a
`TextureView`; this permits normal view composition and transforms.

Glimm transitions affect the GPU content. They do not deform native controls.
An arbitrary GPUI `div()` does not become a UIKit or Android widget. For mixed
interfaces, put native chrome, input fields and navigation in the host and
custom visual content in GPUI. A native control over the GPU view consumes its
own touches; only touches delivered to the GPU view enter Rust.

Relevant platform APIs: [UIGlassEffect](https://developer.apple.com/documentation/uikit/uiglasseffect),
[UIViewRepresentable](https://developer.apple.com/documentation/swiftui/uiviewrepresentable),
[TextureView](https://developer.android.com/reference/android/view/TextureView),
[Views in Compose](https://developer.android.com/develop/ui/compose/migrate/interoperability-apis/views-in-compose).

## Run iOS

Requires Xcode with an iOS simulator runtime, `xcodegen`, and the Rust target:

```sh
rustup target add aarch64-apple-ios-sim
bash crates/gpui_mobile_demo/ios/build.sh
xcrun simctl list devices available
# Use an available simulator UUID below.
xcrun simctl boot "$SIMULATOR_ID"
xcrun simctl install "$SIMULATOR_ID" target/mobile-ios/Build/Products/Debug-iphonesimulator/GpuimmMobile.app
xcrun simctl launch "$SIMULATOR_ID" dev.gpuimm.mobile
bash crates/gpui_mobile_demo/ios/build.sh test \
  -destination "platform=iOS Simulator,id=$SIMULATOR_ID" \
  -resultBundlePath target/mobile-tests.xcresult
```

On Intel Macs, set `GPUIMM_RUST_TARGET=x86_64-apple-ios`. For a device build use
`GPUIMM_RUST_TARGET=aarch64-apple-ios`; the script builds without signing, so
configure your own team/signing before installing on a physical device.
The deployment target is iOS 16. Build with an iOS 26+ SDK for the glass APIs.

For an existing app, build your Rust application crate as a `staticlib`, link it
and the frameworks listed in `ios/project.yml`, import `include/gpuimm.h` via a
bridging header, and include `ios/GpuimmView.swift`. Replace the demo model and
ABI methods with your own application actions. `GpuimmCanvas` is the included
`UIViewRepresentable` wrapper for SwiftUI; put native controls beside it in a
`ZStack`. Keep the native view identity stable while it is displayed.

## Run Android

The supplied script builds an arm64 debug APK without Gradle. It currently
targets macOS build hosts and expects Android platform/build-tools 36 and NDK
27.3.13750724. Override `ANDROID_HOME` / `ANDROID_NDK_HOME` for your installation.
It requires a JDK and `rustup target add aarch64-linux-android`.

```sh
bash crates/gpui_mobile_demo/android/build.sh
adb -s "$ANDROID_SERIAL" install -r target/mobile-android/gpuimm.apk
adb -s "$ANDROID_SERIAL" shell am start -n dev.gpuimm/.DemoActivity
# Explicit GLES override for emulators with broken Vulkan shader translation:
adb -s "$ANDROID_SERIAL" shell am force-stop dev.gpuimm
adb -s "$ANDROID_SERIAL" shell am start -n dev.gpuimm/.DemoActivity --es gpu-backend gl
adb -s "$ANDROID_SERIAL" logcat -s gpuimm
# Instrumentation exercises real GPU output and Surface replacement:
adb -s "$ANDROID_SERIAL" shell am instrument -w \
  -e gpu-backend gl dev.gpuimm/.MobileSmokeTest
```

For an existing Gradle app, place the Rust `.so` in `src/main/jniLibs/arm64-v8a`,
include the two `Gpuimm*.java` files, and preserve their JNI package/class and
native method names through R8. If renaming them, change the Rust JNI exports
too. The example uses Android API 26 minimum and 16 KB library alignment.

In Compose, construct `GpuimmView` inside `AndroidView(factory = { ... })`.
Drive `setActive` from the owning lifecycle's resume/pause events, forward
memory pressure, and call `dispose()` from `onRelease`. Native Compose controls
can be siblings above it in a `Box`. The Java host is tested; a Compose sample
application is not included.

## Rust contract

Use `MobilePlatform::new(native_surface, viewport, text_system)` and pass the
returned `Rc` to `Application::with_platform(...).run_embedded(...)`. Keep both
the platform and returned `ApplicationHandle` alive. Open one GPUI window per
platform. `new_with_backend` permits explicit `GraphicsBackend::{Metal,Vulkan,OpenGl}`;
`Auto` uses Metal on iOS and WGPU's Vulkan/GLES selection on Android.
Android GLES 3.0 uses the renderer's existing texture-based instance transport,
so vertex storage buffers and compute support are not required.

The native surface constructors and platform construction are unsafe because
the caller owns the native pointer. On iOS it must be a live `UIView` whose
backing layer is a `CAMetalLayer`. On Android acquire an `ANativeWindow` from
`Surface` and retain it until after `detach_surface` or platform destruction.
The example JNI bridge owns and releases that reference in the required order.

- All host calls run on the creating UI thread, outside `ApplicationHandle::update`.
- `tick()` drains foreground tasks and requests a frame. Drive it with
  `CADisplayLink` / `Choreographer`, pausing the native frame callback in the
  background. GPUI retains its normal frame invalidation behavior.
- `Viewport` contains physical pixel width/height and pixels-per-point/dp scale.
  Touch positions use local logical points/dp. Resize after bounds or density
  changes. Zero dimensions suspend drawing without configuring an empty swapchain.
- Forward lifecycle, appearance, reduced motion, insets and memory pressure.
  Touches are cancelled when the view is hidden or its surface is detached.
- Detach before an Android `SurfaceTexture` is released. Attach the replacement
  to the same platform to keep the application, device and glyph atlas.
  Destroying the entire host view destroys its model; persist application state
  separately if it must survive Activity/process recreation.
- The example C ABI uses thread-local opaque handles, validates handles and
  buffers, and catches unwind panics at native boundaries. A failed GPU operation
  disposes the demo instance and displays an error. Process-aborting panics,
  native-driver faults and invalid caller-owned pointers cannot be caught.
- Device loss is reported, not silently recovered. Recreate the application
  from your persisted model. Temporary surface loss does not require recreation.
- Call `shutdown()` before dropping the application handle permanently. It
  detaches the surface and cancels queued work to break executor ownership cycles.

## Current scope

This is an embedding backend, not complete mobile parity for every desktop
`Platform` method. Native navigation, dialogs, URLs, clipboard, credentials,
file pickers and menus belong to the host. Unsupported result-returning
operations report failure; desktop-only setters are no-ops.

The GPU subtree has no native accessibility bridge or system IME implementation
in this backend. Use native input and accessible controls, as the demos do.
`with_input_handler` exposes GPUI's input handler for applications implementing
a full native text adapter. Standalone `gpui_ios` remains available for its
existing iOS text-input integration. Native sibling widgets cannot be inserted
between arbitrary GPUI draw primitives with this adapter.

Fonts are explicit: the demo embeds Inter through `CosmicTextSystem` on both
platforms. Products must add their required fonts and fallback coverage. The
renderer and application still need device-specific profiling before shipping.

## Validation

`cargo test -p gpui_mobile -p gpui_mobile_demo --lib` covers callback reentrancy,
viewport validation, dispatcher shutdown and malformed native calls.
`script/check-upstream` checks the upstream hook constraints.

The iOS UI test exercises the native action, native text field, landscape /
portrait changes and background / foreground transitions. It is intended to run
against the actual linked Rust library, not a mock. Build and runtime validation
details for this branch are recorded in the accompanying change description.

Validated on an Apple Silicon Mac with Xcode 27.2 / iOS 27.2 simulator:
iPhone 18 Pro UI test passed, the Metal scene and UIKit glass were visually
inspected, and the arm64 iOS device target passed `cargo check`. On Android
API 36 arm64, the GLES host-GPU emulator passed the instrumentation test,
including actual GPU pixel checks and surface replacement. A touch on the
GPU-rendered button was also checked separately.

This emulator's Vulkan implementation fails while translating the GPUI gradient
shader to Metal; its bundled SwiftShader also rejects a SPIR-V source-language
operand. The explicit GLES override is therefore required on that environment.
Android debug object labels are suppressed to avoid a separate ranchu Vulkan
driver crash, while WGPU validation remains enabled. Vulkan physical-device
runtime behavior, physical iPhone behavior, performance and power consumption
have not been validated by these simulator tests.
