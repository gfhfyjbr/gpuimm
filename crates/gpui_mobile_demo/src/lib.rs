//! Example application and native ABI. Product applications replace this crate's
//! view/model while reusing gpui_mobile and the Swift/Kotlin host views.

#[cfg(target_os = "android")]
mod android;

use anyhow::{Result, anyhow, ensure};
use gpui::{
    AppContext, AppLifecyclePhase, Application, ApplicationHandle, Context, GlimmController,
    GlimmDemo, InteractiveElement, IntoElement, ParentElement, PlatformTextSystem, Render,
    StatefulInteractiveElement, Styled, TouchEvent, TouchId, TouchPhase, Window, WindowAppearance,
    WindowHandle, WindowOptions, div, glimm_scope, point, px, rgb,
};
use gpui_mobile::{CosmicTextSystem, GraphicsBackend, MobilePlatform, NativeSurface, Viewport};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

struct Demo {
    count: i32,
    title: String,
    transition: GlimmController,
}
impl Demo {
    fn advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.transition
            .play(GlimmDemo::Generate.options(), window, cx);
        self.count = self.count.saturating_add(1);
        cx.notify();
    }
}
impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = if self.count % 2 == 0 {
            [0x245547, 0xa9cbb7, 0xe6b85c]
        } else {
            [0x9b4739, 0xe4b09a, 0x455b78]
        };
        let content = div()
            .id("gpu-content")
            .size_full()
            .p_8()
            .pb_32()
            .flex()
            .flex_col()
            .gap_6()
            .bg(rgb(0xf4f2ed))
            .text_color(rgb(0x1d2524))
            .font_family("Inter")
            .child(div().text_sm().child("GPUIMM / MOBILE"))
            .child(div().text_3xl().child(self.title.clone()))
            .child(div().text_xl().child(format!("Scene {}", self.count)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_3()
                    .children(colors.into_iter().enumerate().map(|(index, color)| {
                        div()
                            .flex_1()
                            .h(px(150. + index as f32 * 45.))
                            .rounded_2xl()
                            .bg(rgb(color))
                    })),
            )
            .child(
                div()
                    .id("advance-gpu")
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x245547))
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(|this, _, window, cx| this.advance(window, cx)))
                    .child("Change GPU scene"),
            );
        glimm_scope("mobile-scene", &self.transition, content)
    }
}

struct Instance {
    app: ApplicationHandle,
    platform: Rc<MobilePlatform>,
    window: WindowHandle<Demo>,
    #[cfg(target_os = "android")]
    native_window: Option<android::NativeWindow>,
}
impl Drop for Instance {
    fn drop(&mut self) {
        self.platform.shutdown();
    }
}
thread_local! { static INSTANCES: RefCell<HashMap<u64, Instance>> = RefCell::default(); }
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
unsafe fn create(
    surface: NativeSurface,
    viewport: Viewport,
    backend: GraphicsBackend,
) -> Result<Instance> {
    let text = Arc::new(CosmicTextSystem::new_without_system_fonts("Inter"));
    text.add_fonts(vec![Cow::Borrowed(include_bytes!(
        "../../gpui_perf/src/fast/glimm_assets/Inter.ttf"
    ))])?;
    let platform = unsafe { MobilePlatform::new_with_backend(surface, viewport, text, backend)? };
    let opened = Rc::new(RefCell::new(None));
    let result = opened.clone();
    let app = Application::with_platform(platform.clone()).run_embedded(move |cx| {
        *result.borrow_mut() = Some(cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| Demo {
                count: 0,
                title: "Native controls,\nGPU content.".into(),
                transition: GlimmController::new(),
            })
        }));
    });
    let window = opened
        .borrow_mut()
        .take()
        .ok_or_else(|| anyhow!("launch callback did not run"))??;
    Ok(Instance {
        app,
        platform,
        window,
        #[cfg(target_os = "android")]
        native_window: None,
    })
}

#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
fn insert(instance: Instance) -> Result<u64> {
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    ensure!(
        handle != 0 && handle <= i64::MAX as u64,
        "native handle space exhausted"
    );
    INSTANCES.with(|instances| -> Result<()> {
        instances.try_borrow_mut()?.insert(handle, instance);
        Ok(())
    })?;
    Ok(handle)
}

fn with_instance<R>(handle: u64, f: impl FnOnce(&mut Instance) -> Result<R>) -> Result<R> {
    INSTANCES.with(|instances| {
        let mut instances = instances
            .try_borrow_mut()
            .map_err(|_| anyhow!("reentrant native call"))?;
        let instance = instances
            .get_mut(&handle)
            .ok_or_else(|| anyhow!("invalid handle or wrong UI thread"))?;
        f(instance)
    })
}

fn boundary<T>(fallback: T, f: impl FnOnce() -> Result<T>) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            report_error(&format!("{error:#}"));
            fallback
        }
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            report_error(&format!(
                "panic at native boundary; dispose this instance: {message}"
            ));
            fallback
        }
    }
}

fn report_error(message: &str) {
    #[cfg(target_os = "android")]
    android::write_log(6, message);
    #[cfg(not(target_os = "android"))]
    eprintln!("gpuimm: {message}");
}

/// # Safety
/// The UIView is live, main-thread owned, and backed by CAMetalLayer until destroy.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpuimm_create_ios(
    view: *mut std::ffi::c_void,
    width: u32,
    height: u32,
    scale: f32,
) -> u64 {
    boundary(0, || {
        let view = std::ptr::NonNull::new(view).ok_or_else(|| anyhow!("null UIView"))?;
        let instance = unsafe {
            create(
                NativeSurface::ui_view(view),
                Viewport::new(width, height, scale)?,
                GraphicsBackend::Auto,
            )?
        };
        insert(instance)
    })
}

/// Returns zero on success and -1 on invalid/reentrant/wrong-thread calls.
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_destroy(handle: u64) -> i32 {
    boundary(-1, || {
        INSTANCES.with(|instances| {
            let instance = instances
                .try_borrow_mut()?
                .remove(&handle)
                .ok_or_else(|| anyhow!("invalid handle"))?;
            drop(instance);
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_tick(handle: u64) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance.platform.tick()?;
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_resize(handle: u64, width: u32, height: u32, scale: f32) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance
                .platform
                .resize(Viewport::new(width, height, scale)?)?;
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_lifecycle(handle: u64, active: bool) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance.platform.set_lifecycle(if active {
                AppLifecyclePhase::Active
            } else {
                AppLifecyclePhase::Background
            });
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_appearance(handle: u64, dark: bool, reduce_motion: bool) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance.platform.set_appearance(if dark {
                WindowAppearance::Dark
            } else {
                WindowAppearance::Light
            });
            instance
                .app
                .update(|cx| cx.set_reduce_motion(reduce_motion));
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_memory_warning(handle: u64) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance.platform.memory_warning();
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_advance(handle: u64) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance.app.update(|cx| {
                instance.window.update(cx, |view, window, cx| {
                    view.advance(window, cx);
                    view.count
                })
            })
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_count(handle: u64) -> i32 {
    boundary(-1, || {
        with_instance(handle, |instance| {
            instance
                .app
                .update(|cx| Ok(instance.window.read(cx)?.count))
        })
    })
}

fn set_title(handle: u64, title: String) -> Result<i32> {
    ensure!(title.len() <= 65536, "title too long");
    with_instance(handle, |instance| {
        instance.app.update(|cx| {
            instance.window.update(cx, |view, _, cx| {
                view.title = title;
                cx.notify();
                0
            })
        })
    })
}
/// # Safety
/// `bytes` points to `length` readable UTF-8 bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpuimm_set_title(handle: u64, bytes: *const u8, length: usize) -> i32 {
    boundary(-1, || {
        ensure!(
            length <= 65536 && (!bytes.is_null() || length == 0),
            "invalid title buffer"
        );
        let bytes = if length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(bytes, length) }
        };
        set_title(handle, std::str::from_utf8(bytes)?.to_owned())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn gpuimm_touch(handle: u64, id: u64, phase: u32, x: f32, y: f32) -> i32 {
    boundary(-1, || {
        ensure!(x.is_finite() && y.is_finite(), "invalid touch coordinates");
        let phase = match phase {
            0 => TouchPhase::Started,
            1 => TouchPhase::Moved,
            2 => TouchPhase::Ended,
            3 => TouchPhase::Cancelled,
            _ => return Err(anyhow!("invalid touch phase")),
        };
        with_instance(handle, |instance| {
            instance.platform.touch(TouchEvent {
                id: TouchId(id),
                phase,
                position: point(px(x), px(y)),
                predicted_position: None,
                force: None,
            });
            Ok(0)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{boundary, gpuimm_destroy, gpuimm_set_title, gpuimm_tick, gpuimm_touch};
    #[test]
    fn invalid_handles_and_input_do_not_cross_ffi_with_a_panic() {
        assert_eq!(gpuimm_tick(0), -1);
        assert_eq!(gpuimm_destroy(u64::MAX), -1);
        assert_eq!(gpuimm_touch(0, 1, 9, 0., 0.), -1);
        assert_eq!(unsafe { gpuimm_set_title(0, std::ptr::null(), 1) }, -1);
        assert_eq!(
            boundary(-1, || -> anyhow::Result<i32> { panic!("test") }),
            -1
        );
    }
}
