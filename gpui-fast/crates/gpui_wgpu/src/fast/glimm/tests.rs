use crate::WgpuHeadlessRenderer;
use gpui::{DevicePixels, PlatformHeadlessRenderer as _, Scene, size};

fn renderer() -> WgpuHeadlessRenderer {
    WgpuHeadlessRenderer::new().expect("Glimm tests require a WGPU device")
}
fn render(renderer: &mut WgpuHeadlessRenderer, scene: &Scene) -> image::RgbaImage {
    renderer
        .render_scene_to_image(scene, size(DevicePixels(512), DevicePixels(320)))
        .unwrap()
}
fn cache_counts(renderer: &WgpuHeadlessRenderer) -> (bool, usize, usize) {
    let state = &renderer.core.fast_glimm;
    assert_eq!(state.current.len(), state.old.len());
    (state.live.is_some(), state.old.len(), state.outputs.len())
}
fn backend_name() -> &'static str {
    "wgpu"
}
fn shader() -> &'static str {
    include_str!("../glimm.wgsl")
}
fn solid_green() -> (&'static str, &'static str) {
    (
        "let result = band_color + page_color * (1.0 - band_color.a);",
        "let result = vec4(0.0, 1.0, 0.0, 1.0);",
    )
}
fn follow(renderer: &mut WgpuHeadlessRenderer, path: std::path::PathBuf) {
    renderer.core.fast_glimm.source =
        Some(gpui::GlimmShaderSource::following(Some(path), shader()));
}

#[path = "../../../../gpui/src/fast/glimm/gpu_tests.rs"]
mod shared;

#[test]
fn glimm_gpu_reuses_cropped_live_and_snapshot_textures_until_idle() {
    let mut renderer = renderer();
    let scene = shared::parity_scene();
    render(&mut renderer, &scene);
    let live = renderer.core.fast_glimm.current[0].1.texture.clone();
    let old = renderer.core.fast_glimm.old[0].1.texture.clone();
    let output = renderer.core.fast_glimm.outputs[0].1.texture.clone();
    render(&mut renderer, &scene);
    assert_eq!(live, renderer.core.fast_glimm.current[0].1.texture);
    assert_eq!(old, renderer.core.fast_glimm.old[0].1.texture);
    assert_eq!(output, renderer.core.fast_glimm.outputs[0].1.texture);
    render(&mut renderer, &Scene::default());
    assert_eq!(cache_counts(&renderer), (false, 0, 0));
}

#[test]
fn glimm_gpu_resize_recreates_snapshots_and_restores_the_window_pipeline() {
    let mut renderer = renderer();
    let scene = shared::parity_scene();
    let first = render(&mut renderer, &scene);
    let second = renderer
        .render_scene_to_image(&scene, size(DevicePixels(768), DevicePixels(480)))
        .unwrap();
    assert_eq!(renderer.core.fast_glimm.viewport, [768, 480]);
    assert_eq!(cache_counts(&renderer), (true, 1, 1));
    for (x, y) in [(100, 100), (250, 150), (400, 280)] {
        for channel in 0..4 {
            assert!(first.get_pixel(x, y)[channel].abs_diff(second.get_pixel(x, y)[channel]) <= 1);
        }
    }
}
