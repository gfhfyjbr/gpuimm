use super::{boundary, create, insert, with_instance};
use anyhow::{Result, anyhow};
use gpui_mobile::{NativeSurface, Viewport};
use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jboolean, jfloat, jint, jlong, jobject},
};
use std::{ffi::c_void, ptr::NonNull};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}

pub(super) fn write_log(priority: i32, message: &str) {
    if let Ok(message) = std::ffi::CString::new(message.replace('\0', "�")) {
        unsafe {
            __android_log_write(priority, c"gpuimm".as_ptr(), message.as_ptr());
        }
    }
}

struct AndroidLogger;
impl log::Log for AndroidLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            write_log(
                match record.level() {
                    log::Level::Error => 6,
                    log::Level::Warn => 5,
                    _ => 4,
                },
                &format!("{}: {}", record.target(), record.args()),
            );
        }
    }
    fn flush(&self) {}
}
fn init_logging() {
    static INIT: std::sync::Once = std::sync::Once::new();
    static LOGGER: AndroidLogger = AndroidLogger;
    INIT.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    });
}

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut jni::sys::JNIEnv, surface: jobject) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
}

pub(super) struct NativeWindow(NonNull<c_void>);
impl NativeWindow {
    fn from_surface(env: &JNIEnv, surface: &JObject) -> Result<Self> {
        let window =
            unsafe { ANativeWindow_fromSurface(env.get_native_interface(), surface.as_raw()) };
        NonNull::new(window)
            .map(Self)
            .ok_or_else(|| anyhow!("Surface has no ANativeWindow"))
    }
    unsafe fn target(&self) -> NativeSurface {
        unsafe { NativeSurface::android_window(self.0) }
    }
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.0.as_ptr());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_attach(
    env: JNIEnv,
    _: JClass,
    handle: jlong,
    surface: JObject,
    width: jint,
    height: jint,
    scale: jfloat,
    backend: jint,
) -> jlong {
    init_logging();
    boundary(0, || {
        let viewport = Viewport::new(width.try_into()?, height.try_into()?, scale)?;
        let native = NativeWindow::from_surface(&env, &surface)?;
        if handle == 0 {
            let backend = match backend {
                0 => gpui_mobile::GraphicsBackend::Auto,
                1 => gpui_mobile::GraphicsBackend::Vulkan,
                2 => gpui_mobile::GraphicsBackend::OpenGl,
                _ => anyhow::bail!("invalid Android graphics backend"),
            };
            let mut instance = unsafe { create(native.target(), viewport, backend)? };
            instance.native_window = Some(native);
            Ok(insert(instance)? as jlong)
        } else {
            with_instance(handle as u64, |instance| {
                unsafe {
                    instance
                        .platform
                        .attach_surface(native.target(), viewport)?;
                }
                instance.native_window = Some(native);
                Ok(handle)
            })
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_detach(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    boundary(-1, || {
        with_instance(handle as u64, |instance| {
            instance.platform.detach_surface();
            instance.native_window.take();
            Ok(0)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_destroy(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    super::gpuimm_destroy(handle as u64)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_tick(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    super::gpuimm_tick(handle as u64)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_resize(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    width: jint,
    height: jint,
    scale: jfloat,
) -> jint {
    if width < 0 || height < 0 {
        return -1;
    }
    super::gpuimm_resize(handle as u64, width as u32, height as u32, scale)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_lifecycle(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    active: jboolean,
) -> jint {
    super::gpuimm_lifecycle(handle as u64, active != 0)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_appearance(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    dark: jboolean,
    reduced: jboolean,
) -> jint {
    super::gpuimm_appearance(handle as u64, dark != 0, reduced != 0)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_memoryWarning(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    super::gpuimm_memory_warning(handle as u64)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_advance(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    super::gpuimm_advance(handle as u64)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_count(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
) -> jint {
    super::gpuimm_count(handle as u64)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_title(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    title: JString,
) -> jint {
    boundary(-1, || {
        let text: String = env.get_string(&title)?.into();
        super::set_title(handle as u64, text)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_gpuimm_GpuimmNative_touch(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    id: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
) -> jint {
    super::gpuimm_touch(handle as u64, id as u64, phase as u32, x, y)
}
