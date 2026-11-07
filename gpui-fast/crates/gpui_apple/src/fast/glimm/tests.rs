use crate::metal_renderer::{InstanceBufferPool, MetalRenderer};
use gpui::{DevicePixels, Scene, size};
use parking_lot::Mutex;
use std::sync::Arc;

fn renderer() -> MetalRenderer {
    MetalRenderer::new_headless(Arc::new(Mutex::new(InstanceBufferPool::default())))
        .expect("Glimm tests require a Metal device")
}
fn render(renderer: &mut MetalRenderer, scene: &Scene) -> image::RgbaImage {
    renderer
        .render_scene_to_image(scene, size(DevicePixels(512), DevicePixels(320)))
        .unwrap()
}
fn cache_counts(renderer: &MetalRenderer) -> (bool, usize, usize) {
    let state = &renderer.fast_glimm;
    (state.live.is_some(), state.old.len(), state.outputs.len())
}
fn backend_name() -> &'static str {
    "metal"
}

#[path = "../../../../gpui/src/fast/glimm/gpu_tests.rs"]
mod shared;
