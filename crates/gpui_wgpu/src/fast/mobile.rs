pub(crate) fn backends() -> wgpu::Backends {
    if cfg!(target_os = "ios") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
}

pub(crate) fn instance_flags() -> wgpu::InstanceFlags {
    let flags = wgpu::InstanceFlags::default();
    if cfg!(target_os = "android") {
        // Android 36's ranchu Vulkan driver crashes in SetDebugUtilsObjectNameEXT
        // while creating wgpu's internal descriptor layouts. Keep validation but
        // avoid driver object labels, including wgpu's internally generated ones.
        flags | wgpu::InstanceFlags::DISCARD_HAL_LABELS
    } else {
        flags
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn native_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    let base = if cfg!(target_os = "android") && adapter.get_info().backend == wgpu::Backend::Gl {
        // GLES 3.0 has neither compute nor vertex storage buffers. Reuse the
        // renderer's existing texture transport instead of requiring GLES 3.1.
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::downlevel_defaults()
    };
    base.using_resolution(adapter.limits())
        .using_alignment(adapter.limits())
}

pub(crate) fn uses_texture_instances(backend: crate::WgpuBackend) -> bool {
    (cfg!(target_family = "wasm") && matches!(backend, crate::WgpuBackend::Gl))
        || (cfg!(target_os = "android")
            && matches!(backend, crate::WgpuBackend::Native(wgpu::Backend::Gl)))
}
