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
fn shader() -> &'static str {
    include_str!("../glimm.metal")
}
fn solid_green() -> (&'static str, &'static str) {
    (
        "float4 result = band+content*(1.0-band.a);",
        "float4 result = float4(0,1,0,1);",
    )
}
fn follow(renderer: &mut MetalRenderer, path: std::path::PathBuf) {
    renderer.fast_glimm.source = Some(gpui::GlimmShaderSource::following(Some(path), shader()));
}

#[path = "../../../../gpui/src/fast/glimm/gpu_tests.rs"]
mod shared;

/// What a radial distortion costs on a scope the size of a 3440 × 1440 display, next to the
/// same frame without an effect and with the glimm.dev sweep. Wall time per frame, readback
/// included; run with `cargo test -p gpui_apple --features test-support --lib glimm_cost --
/// --ignored --nocapture`, ideally in a release build.
#[test]
#[ignore = "a timing, not a check"]
fn glimm_cost_of_a_display_sized_scope() {
    use gpui::{
        Bounds, ContentMask, GlimmFrame, GlimmOptions, GlimmTimeline, GlimmUniforms, Quad,
        ScaledPixels, point, rgb,
    };
    use std::{rc::Rc, time::Instant};
    let (w, h) = (3440., 1440.);
    let full = Bounds::new(
        point(ScaledPixels(0.), ScaledPixels(0.)),
        size(ScaledPixels(w), ScaledPixels(h)),
    );
    let checker = || {
        let mut scene = Scene::default();
        for y in 0..(h as usize / 32) {
            for x in 0..(w as usize / 32) {
                scene.insert_primitive(Quad {
                    bounds: Bounds::new(
                        point(ScaledPixels(x as f32 * 32.), ScaledPixels(y as f32 * 32.)),
                        size(ScaledPixels(32.), ScaledPixels(32.)),
                    ),
                    content_mask: ContentMask { bounds: full },
                    background: rgb(if (x + y) % 2 == 0 { 0xffffff } else { 0x242529 }).into(),
                    ..Default::default()
                });
            }
        }
        scene.finish();
        scene
    };
    let mut renderer = renderer();
    let mut time = |name: &str, frame: Option<(GlimmOptions, f32)>| {
        let mut scene = checker();
        if let Some((options, at_ms)) = frame {
            let sample = GlimmTimeline::new(options.clone(), false).sample(at_ms);
            scene.fast_glimm.frames.push(GlimmFrame {
                run_id: 1,
                bounds: full,
                clip: full,
                paint_range: 0..scene.len(),
                old_scene: Rc::new(checker()),
                uniforms: GlimmUniforms::for_scope(&options, sample, [w, h], 1.),
            });
        }
        let size = size(DevicePixels(w as i32), DevicePixels(h as i32));
        renderer.render_scene_to_image(&scene, size).unwrap();
        let started = Instant::now();
        for _ in 0..10 {
            renderer.render_scene_to_image(&scene, size).unwrap();
        }
        let ms = started.elapsed().as_secs_f32() * 100.;
        eprintln!("{name}: {ms:.2} ms a frame");
        ms
    };
    let centre = Some([w / 2., h / 2.]);
    time("no effect", None);
    time(
        "shock, no amplitude (one tap a pixel)",
        Some((
            GlimmOptions {
                at: centre,
                amplitude: 0.,
                ..GlimmOptions::shock()
            },
            300.,
        )),
    );
    time(
        "shock at 300ms",
        Some((
            GlimmOptions {
                at: centre,
                ..GlimmOptions::shock()
            },
            300.,
        )),
    );
    time(
        "emerge at 300ms",
        Some((
            GlimmOptions {
                at: centre,
                ..GlimmOptions::emerge()
            },
            300.,
        )),
    );
    time(
        "collapse at 500ms",
        Some((
            GlimmOptions {
                at: centre,
                ..GlimmOptions::collapse()
            },
            500.,
        )),
    );
    time(
        "glimm.dev scope at 800ms",
        Some((GlimmOptions::default(), 800.)),
    );
}
