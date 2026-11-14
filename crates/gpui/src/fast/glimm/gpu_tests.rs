//! The same scoped-composition and original-WebGL oracle tests run on each renderer.
use gpui::{
    Bounds, ContentMask, GlimmFrame, GlimmOptions, GlimmTimeline, GlimmUniforms, Quad,
    ScaledPixels, Scene, point, rgb, size,
};
use std::rc::Rc;

use super::{backend_name, cache_counts, follow, render, renderer, shader, solid_green};

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}
fn quad(scene: &mut Scene, b: Bounds<ScaledPixels>, color: u32) {
    scene.insert_primitive(Quad {
        bounds: b,
        content_mask: ContentMask {
            bounds: bounds(0., 0., 512., 320.),
        },
        background: rgb(color).into(),
        ..Default::default()
    });
}
fn pattern(invert: bool) -> Scene {
    let mut scene = Scene::default();
    for y in 0..40 {
        for x in 0..64 {
            let light = (x + y) % 2 == usize::from(invert);
            quad(
                &mut scene,
                bounds((x * 8) as f32, (y * 8) as f32, 8., 8.),
                if light { 0xffffff } else { 0x242529 },
            );
        }
    }
    scene.finish();
    scene
}
fn solid(color: u32) -> Scene {
    let mut scene = Scene::default();
    quad(&mut scene, bounds(0., 0., 512., 320.), color);
    scene.finish();
    scene
}
fn frame(old: Scene, dir: [f32; 2], gather: f32, ring: f32) -> GlimmFrame {
    let length = if dir[0] != 0. { 512. } else { 320. };
    let mut u = GlimmUniforms::default();
    u.0[0] = [512., 320., 1., 1.25];
    u.0[2] = [256., 160., dir[0], dir[1]];
    u.0[3] = [0.5, 0., gather, 0.];
    u.0[4] = [ring, 1., 0., 1.];
    u.0[5] = [28., 0., 1., 0.5];
    u.0[6] = [0., 1., 1., 1.15];
    u.0[7] = [0.15, 0., 0., 1.];
    u.0[8] = [-0.4, 1.4, length / 2., 0.5 * length / 28f32.sqrt()];
    u.0[9] = [0.74, 0.72, 0.76, 0.];
    u.0[10] = [0.5, 0.52, 0.48, 0.];
    u.0[11] = [1., 1., 1., 0.];
    u.0[12] = [0.92, 0.25, 0.58, 0.];
    GlimmFrame {
        run_id: 1,
        bounds: bounds(0., 0., 512., 320.),
        clip: bounds(0., 0., 512., 320.),
        paint_range: 0..old.len(),
        old_scene: Rc::new(old),
        uniforms: u,
    }
}
#[test]
fn glimm_gpu_old_new_split_follows_all_four_directions() {
    let mut renderer = renderer();
    for (id, dir) in [[1., 0.], [-1., 0.], [0., 1.], [0., -1.]]
        .into_iter()
        .enumerate()
    {
        let mut scene = solid(0xffffff);
        let mut f = frame(solid(0x000000), dir, 0., 0.);
        f.run_id = id as u64 + 1;
        scene.fast_glimm.frames.push(f);
        let image = render(&mut renderer, &scene);
        let ahead = if dir[0] != 0. {
            image.get_pixel(if dir[0] > 0. { 480 } else { 32 }, 160)
        } else {
            image.get_pixel(256, if dir[1] > 0. { 300 } else { 20 })
        };
        let behind = if dir[0] != 0. {
            image.get_pixel(if dir[0] > 0. { 32 } else { 480 }, 160)
        } else {
            image.get_pixel(256, if dir[1] > 0. { 20 } else { 300 })
        };
        assert!(ahead[0] < 4, "old page ahead of {dir:?}: {ahead:?}");
        assert!(behind[0] > 250, "new page behind {dir:?}: {behind:?}");
    }
}
#[test]
fn glimm_gpu_gather_deforms_the_actual_content_and_idle_frees_textures() {
    let mut renderer = renderer();
    let base = pattern(false);
    let before = render(&mut renderer, &base);
    let mut scene = pattern(false);
    let mut f = frame(pattern(false), [0., 1.], 1., 0.);
    f.uniforms.0[4][3] = 0.;
    scene.fast_glimm.frames.push(f);
    let after = render(&mut renderer, &scene);
    let changed = before
        .pixels()
        .zip(after.pixels())
        .filter(|(a, b)| a[0].abs_diff(b[0]) > 20)
        .count();
    assert!(
        changed > 10000,
        "content was not spatially sampled: {changed} changed pixels"
    );
    assert!(cache_counts(&renderer).0);
    render(&mut renderer, &base);
    assert_eq!(cache_counts(&renderer), (false, 0, 0));
}
#[test]
fn glimm_gpu_clip_leaves_pixels_outside_the_scope_unchanged() {
    let mut renderer = renderer();
    let mut scene = solid(0xffffff);
    quad(&mut scene, bounds(100., 80., 300., 160.), 0xffffff);
    scene.finish();
    let mut f = frame(solid(0x000000), [0., 1.], 0., 1.);
    f.paint_range = 1..2;
    f.clip = bounds(100., 80., 300., 160.);
    scene.fast_glimm.frames.push(f);
    let image = render(&mut renderer, &scene);
    for (x, y) in [(10, 10), (500, 310), (90, 160), (410, 160)] {
        assert_eq!(image.get_pixel(x, y).0, [255; 4]);
    }
}
pub(super) fn parity_scene() -> Scene {
    let mut scene = pattern(false);
    let mut f = frame(pattern(true), [0., 1.], 0.7, 0.85);
    f.uniforms.0[3] = [0.45, 0.7, 0.7, 0.5];
    f.uniforms.0[5] = [28., 0.22, 0.64, 0.5];
    f.uniforms.0[8] = [
        1.4053212,
        -0.4053212,
        0.45 * 1.8106424 * 320. - 0.4053212 * 320.,
        0.5 * 320. / 28f32.sqrt(),
    ];
    f.uniforms.0[1][2] = 512.;
    f.uniforms.0[1][3] = 320.;
    scene.fast_glimm.frames.push(f);
    scene
}
#[test]
fn glimm_gpu_matches_the_original_webgl_shader() {
    let expected = image::load_from_memory(include_bytes!(
        "../../../../gpui_apple/src/fast/glimm/reference.png"
    ))
    .unwrap()
    .to_rgba8();
    let actual = render(&mut renderer(), &parity_scene());
    let mut sum = 0u64;
    let mut max = 0;
    for (a, b) in actual.pixels().zip(expected.pixels()) {
        for i in 0..3 {
            let d = a[i].abs_diff(b[i]);
            sum += u64::from(d);
            max = max.max(d);
        }
    }
    let mean = sum as f64 / (512. * 320. * 3.);
    eprintln!("WebGL/{} parity: mean {mean}, max {max}", backend_name());
    assert!(
        mean < 0.5 && max <= 4,
        "WebGL/{} parity: mean {mean}, max {max}",
        backend_name()
    );
}
#[test]
fn glimm_gpu_write_parity_artifacts() {
    let Some(root) = std::env::var_os("GLIMM_ARTIFACTS") else {
        return;
    };
    let root = std::path::Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    let mut renderer = renderer();
    render(&mut renderer, &pattern(false))
        .save(root.join("glimm-live.png"))
        .unwrap();
    render(&mut renderer, &pattern(true))
        .save(root.join("glimm-old.png"))
        .unwrap();
    let scene = parity_scene();
    let json = format!(
        "[{}]",
        scene.fast_glimm.frames[0]
            .uniforms
            .0
            .iter()
            .map(|r| format!("[{},{},{},{}]", r[0], r[1], r[2], r[3]))
            .collect::<Vec<_>>()
            .join(",")
    );
    std::fs::write(root.join("glimm-uniforms.json"), json).unwrap();
    render(&mut renderer, &scene)
        .save(root.join(format!("glimm-{}.png", backend_name())))
        .unwrap();
}
#[test]
fn glimm_gpu_does_not_deform_an_overlay_painted_after_the_scope() {
    let mut scene = pattern(false);
    let end = scene.len();
    quad(&mut scene, bounds(180., 130., 140., 70.), 0xff0000);
    scene.finish();
    let mut f = frame(pattern(true), [0., 1.], 1., 1.);
    f.paint_range = 0..end;
    scene.fast_glimm.frames.push(f);
    let image = render(&mut renderer(), &scene);
    assert_eq!(image.get_pixel(200, 150).0, [255, 0, 0, 255]);
}
#[test]
fn glimm_gpu_nested_scopes_compose_in_paint_order() {
    let mut scene = pattern(false);
    let inner = frame(pattern(false), [0., 1.], 0.5, 0.5);
    let mut outer = frame(pattern(false), [1., 0.], 0.5, 0.5);
    outer.run_id = 2;
    scene.fast_glimm.frames.extend([inner, outer]);
    let mut renderer = renderer();
    let image = render(&mut renderer, &scene);
    assert!(image.pixels().all(|p| p[3] == 255));
    assert_eq!(cache_counts(&renderer).2, 2);
}
fn scoped_frame(old: Scene, b: Bounds<ScaledPixels>, dpr: f32) -> GlimmFrame {
    let mut f = frame(old, [0., 1.], 0., 0.);
    f.bounds = b;
    f.clip = b;
    f.paint_range = 1..2;
    f.uniforms.0[0] = [b.size.width.0 / dpr, b.size.height.0 / dpr, dpr, 0.];
    f.uniforms.0[1] = [b.origin.x.0, b.origin.y.0, 0., 0.];
    f.uniforms.0[2] = [
        b.size.width.0 / (2. * dpr),
        b.size.height.0 / (2. * dpr),
        0.,
        1.,
    ];
    f.uniforms.0[4][3] = 0.;
    f
}
#[test]
fn glimm_gpu_cropped_mipmaps_do_not_bleed_outside_the_scope() {
    let mut scene = solid(0x0000ff);
    let b = bounds(100., 80., 300., 160.);
    quad(&mut scene, b, 0x00ff00);
    scene.finish();
    let mut f = scoped_frame(solid(0x00ff00), b, 1.);
    f.uniforms.0[3][2] = 1.;
    f.uniforms.0[3][3] = 1.;
    f.uniforms.0[4][0] = 1.;
    scene.fast_glimm.frames.push(f);
    let image = render(&mut renderer(), &scene);
    for (x, y) in [(100, 80), (399, 239), (101, 238), (398, 81), (250, 160)] {
        assert_eq!(
            image.get_pixel(x, y).0,
            [0, 255, 0, 255],
            "scope edge ({x},{y})"
        );
    }
}
#[test]
fn glimm_gpu_transparent_content_is_not_premultiplied_twice() {
    for dpr in [1., 1.5, 2.] {
        let mut renderer = renderer();
        let mut scene = solid(0x00ff00);
        let b = bounds(100., 80., 300., 160.);
        scene.insert_primitive(Quad {
            bounds: b,
            content_mask: ContentMask { bounds: b },
            background: gpui::rgba(0xff000080).into(),
            ..Default::default()
        });
        scene.finish();
        let expected = render(&mut renderer, &scene);
        scene
            .fast_glimm
            .frames
            .push(scoped_frame(solid(0x000000), b, dpr));
        let actual = render(&mut renderer, &scene);
        for i in 0..4 {
            assert!(
                actual.get_pixel(250, 160)[i].abs_diff(expected.get_pixel(250, 160)[i]) <= 1,
                "DPR {dpr}: actual {:?}, expected {:?}",
                actual.get_pixel(250, 160),
                expected.get_pixel(250, 160)
            );
        }
    }
}
#[test]
fn glimm_gpu_a_partly_offscreen_scope_keeps_its_local_origin() {
    let b = bounds(-20., -15., 300., 160.);
    let mut scene = solid(0x0000ff);
    quad(&mut scene, b, 0x00ff00);
    scene.finish();
    scene
        .fast_glimm
        .frames
        .push(scoped_frame(solid(0x00ff00), b, 1.));
    let image = render(&mut renderer(), &scene);
    for (x, y) in [(1, 1), (100, 80), (278, 143)] {
        assert_eq!(image.get_pixel(x, y).0, [0, 255, 0, 255]);
    }
    assert_eq!(image.get_pixel(300, 160).0, [0, 0, 255, 255]);
}
#[test]
fn glimm_gpu_sibling_scopes_do_not_sample_each_other() {
    let mut scene = solid(0x0000ff);
    let left = bounds(20., 20., 180., 260.);
    let right = bounds(260., 20., 180., 260.);
    quad(&mut scene, left, 0xff0000);
    quad(&mut scene, right, 0x00ff00);
    scene.finish();
    let mut a = scoped_frame(solid(0xff0000), left, 1.);
    a.uniforms.0[3][2] = 1.;
    let mut b = scoped_frame(solid(0x00ff00), right, 1.);
    b.run_id = 2;
    b.paint_range = 2..3;
    b.uniforms.0[3][2] = 1.;
    scene.fast_glimm.frames.extend([a, b]);
    let image = render(&mut renderer(), &scene);
    assert_eq!(image.get_pixel(100, 120).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(340, 120).0, [0, 255, 0, 255]);
    assert_eq!(image.get_pixel(230, 120).0, [0, 0, 255, 255]);
}
#[test]
fn glimm_gpu_follows_an_edited_shader_and_keeps_the_last_good_one() {
    let directory = std::env::temp_dir().join(format!(
        "glimm-follow-{}-{}",
        backend_name(),
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("shader");
    let write = |text: &str, seconds: u64| {
        std::fs::write(&path, text).unwrap();
        let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(stamp)
            .unwrap();
    };
    let near = |a: &image::RgbaImage, b: &image::RgbaImage| {
        a.pixels()
            .zip(b.pixels())
            .all(|(a, b)| (0..4).all(|i| a[i].abs_diff(b[i]) <= 1))
    };
    let (needle, green) = solid_green();
    assert!(
        shader().contains(needle),
        "the shader no longer has {needle}"
    );
    let scene = parity_scene();
    let built_in = render(&mut renderer(), &scene);

    write(shader(), 1_000);
    let mut renderer = renderer();
    follow(&mut renderer, path.clone());
    assert!(near(&render(&mut renderer, &scene), &built_in));

    write(&shader().replace(needle, green), 2_000);
    let edited = render(&mut renderer, &scene);
    let centre = edited.get_pixel(256, 160).0;
    assert!(
        centre[0] < 8 && centre[1] > 247 && centre[2] < 8,
        "the edited shader was not compiled: {centre:?}"
    );

    // A shader that does not build leaves the edited one drawing.
    write("this does not compile", 3_000);
    assert!(near(&render(&mut renderer, &scene), &edited));

    write(shader(), 4_000);
    assert!(near(&render(&mut renderer, &scene), &built_in));
    std::fs::remove_dir_all(&directory).unwrap();
}

// Radial distortions, driven through the real settings, timeline and uniforms.

fn radial_frame(live_len: usize, old: Scene, options: &GlimmOptions, at_ms: f32) -> GlimmFrame {
    let timeline = GlimmTimeline::new(options.clone(), false);
    GlimmFrame {
        run_id: 1,
        bounds: bounds(0., 0., 512., 320.),
        clip: bounds(0., 0., 512., 320.),
        paint_range: 0..live_len,
        old_scene: Rc::new(old),
        uniforms: GlimmUniforms::for_scope(options, timeline.sample(at_ms), [512., 320.], 1.),
    }
}
fn radial_render(live: Scene, old: Scene, options: &GlimmOptions, at_ms: f32) -> image::RgbaImage {
    let mut scene = live;
    let frame = radial_frame(scene.len(), old, options, at_ms);
    scene.fast_glimm.frames.push(frame);
    render(&mut renderer(), &scene)
}
fn end_of(options: &GlimmOptions) -> f32 {
    GlimmTimeline::new(options.clone(), false).end_ms().unwrap()
}
/// The largest channel difference over the pixels `keep` selects.
fn largest_difference(
    a: &image::RgbaImage,
    b: &image::RgbaImage,
    keep: impl Fn(u32, u32) -> bool,
) -> u8 {
    a.enumerate_pixels()
        .filter(|(x, y, _)| keep(*x, *y))
        .map(|(x, y, p)| {
            (0..4)
                .map(|i| p[i].abs_diff(b.get_pixel(x, y)[i]))
                .max()
                .unwrap()
        })
        .max()
        .unwrap_or(0)
}
/// A white page with a black vertical line, 4px wide, centred on `x`.
fn line(x: f32) -> Scene {
    let mut scene = Scene::default();
    quad(&mut scene, bounds(0., 0., 512., 320.), 0xffffff);
    quad(&mut scene, bounds(x - 2., 0., 4., 320.), 0x000000);
    scene.finish();
    scene
}
/// Where the line crosses row `y`: the centre of its dark pixels between `from` and `to`.
fn line_centre(image: &image::RgbaImage, y: u32, from: u32, to: u32) -> f32 {
    let (sum, weight) = (from..to).fold((0., 0.), |(sum, weight), x| {
        let darkness = 255. - f32::from(image.get_pixel(x, y)[1]);
        (sum + darkness * x as f32, weight + darkness)
    });
    assert!(weight > 100., "no line on row {y}");
    sum / weight + 0.5
}
/// The first instant at which the front has travelled `radius` logical pixels.
fn when_front_reaches(options: &GlimmOptions, radius: f32) -> f32 {
    let timeline = GlimmTimeline::new(options.clone(), false);
    (0..2000)
        .map(|ms| ms as f32)
        .find(|ms| {
            GlimmUniforms::for_scope(options, timeline.sample(*ms), [512., 320.], 1.).0[13][1]
                >= radius
        })
        .expect("the front reaches that radius")
}

#[test]
fn glimm_gpu_emerge_starts_on_the_old_content_and_ends_exactly_on_the_new() {
    // The point of action lies outside the scope, as on a parent's edge.
    let options = GlimmOptions {
        at: Some([-40., 160.]),
        ..GlimmOptions::emerge()
    };
    let mut plain = renderer();
    let old = render(&mut plain, &pattern(true));
    let new = render(&mut plain, &pattern(false));
    let start = radial_render(pattern(false), pattern(true), &options, 0.);
    assert!(
        largest_difference(&start, &old, |_, _| true) <= 1,
        "Emerge does not start on the old content"
    );
    let end = end_of(&options);
    let last = radial_render(pattern(false), pattern(true), &options, end - 1.);
    assert!(
        largest_difference(&last, &new, |_, _| true) <= 3,
        "the last frame is not the new content"
    );
    let done = radial_render(pattern(false), pattern(true), &options, end);
    assert!(
        largest_difference(&done, &new, |_, _| true) <= 1,
        "displacement or tint left at the end"
    );
}
#[test]
fn glimm_gpu_emerge_brings_new_content_out_of_a_point_outside_the_scope() {
    let options = GlimmOptions {
        at: Some([-40., 160.]),
        ..GlimmOptions::emerge()
    };
    // Mid-way the front has crossed the near side of the scope but not the far side.
    let at_ms = when_front_reaches(&options, 260.);
    let image = radial_render(solid(0x00ff00), solid(0x0000ff), &options, at_ms);
    let near = image.get_pixel(30, 160);
    let far = image.get_pixel(500, 160);
    assert!(
        near[1] > 200 && near[2] < 60,
        "no new content near the point: {near:?}"
    );
    assert!(
        far[2] > 200 && far[1] < 60,
        "new content far from the front: {far:?}"
    );
}
#[test]
fn glimm_gpu_emerge_pushes_the_old_content_outward_and_bends_its_lines() {
    let options = GlimmOptions {
        at: Some([100., 160.]),
        ..GlimmOptions::emerge()
    };
    let at_ms = when_front_reaches(&options, 120.);
    let image = radial_render(solid(0x00ff00), line(300.), &options, at_ms);
    // On the row through the point the push is horizontal and away from the point.
    let level = line_centre(&image, 160, 230, 440) - 300.;
    // Farther from the point it is weaker, so the straight line comes out curved.
    let high = line_centre(&image, 40, 230, 440) - 300.;
    eprintln!(
        "{}: Emerge pushed the line {level}px on the point's row, {high}px 120px above",
        backend_name()
    );
    assert!(
        level > 4.,
        "the old content did not move outward: {level}px"
    );
    assert!(
        level - high > 2.,
        "the line did not bend: {level}px level, {high}px high"
    );
}
#[test]
fn glimm_gpu_shock_bends_the_live_content_and_leaves_it_unchanged_at_its_end() {
    let options = GlimmOptions {
        at: Some([256., 160.]),
        ..GlimmOptions::shock()
    };
    let new = render(&mut renderer(), &pattern(false));
    let at_ms = when_front_reaches(&options, 120.);
    let mid = radial_render(pattern(false), pattern(true), &options, at_ms);
    let bent = new
        .pixels()
        .zip(mid.pixels())
        .filter(|(a, b)| a[0].abs_diff(b[0]) > 20)
        .count();
    eprintln!("{}: Shock moved {bent} pixels", backend_name());
    assert!(bent > 3000, "the ring bent only {bent} pixels");
    // Shock draws what the scope holds now, never the snapshot taken before.
    let ahead = |x: u32, y: u32| (x as f32 - 256.).hypot(y as f32 - 160.) > 240.;
    assert!(
        largest_difference(&mid, &new, ahead) <= 1,
        "content ahead of the ring changed"
    );
    let done = radial_render(pattern(false), pattern(true), &options, end_of(&options));
    assert!(
        largest_difference(&done, &new, |_, _| true) <= 1,
        "Shock left the content changed"
    );
}
#[test]
fn glimm_gpu_collapse_draws_the_old_content_into_the_point_and_gone() {
    let options = GlimmOptions {
        at: Some([256., 160.]),
        ..GlimmOptions::collapse()
    };
    fn card() -> Scene {
        let mut card = solid(0xffffff);
        quad(&mut card, bounds(156., 100., 200., 120.), 0xff0000);
        card.finish();
        card
    }
    let red = |image: &image::RgbaImage| {
        image
            .pixels()
            .filter(|p| p[0] > 200 && p[1] < 90 && p[2] < 90)
            .count()
    };
    let at = |ms: f32| radial_render(solid(0xffffff), card(), &options, ms);
    let whole = red(&render(&mut renderer(), &card()));
    let end = end_of(&options);
    let start = red(&at(0.));
    let middle = red(&at(end * 0.6));
    let done = red(&at(end));
    eprintln!(
        "{}: Collapse left {start}, {middle}, {done} of {whole} card pixels",
        backend_name()
    );
    assert!(
        start + 200 > whole,
        "the card is not whole at the start: {start} of {whole}"
    );
    assert!(
        middle > 0 && middle * 2 < whole,
        "the card was not drawn in: {middle} of {whole}"
    );
    assert_eq!(done, 0, "old content left at the end");
}
#[test]
fn glimm_gpu_hold_breathes_around_the_point_and_leaves_the_rest() {
    let options = GlimmOptions {
        at: Some([256., 160.]),
        hold_radius: 40.,
        ..GlimmOptions::shock()
    };
    let timeline = GlimmTimeline::new(options.clone(), true);
    let breath = options.breath_ms;
    let frame_at = |ms: f32| {
        let mut scene = pattern(false);
        let mut frame = radial_frame(scene.len(), pattern(false), &options, 0.);
        frame.uniforms = GlimmUniforms::for_scope(&options, timeline.sample(ms), [512., 320.], 1.);
        scene.fast_glimm.frames.push(frame);
        render(&mut renderer(), &scene)
    };
    let plain = render(&mut renderer(), &pattern(false));
    let exhaled = frame_at(4. * breath);
    let inhaled = frame_at(4.5 * breath);
    let far = |x: u32, y: u32| (x as f32 - 256.).hypot(y as f32 - 160.) > 5. * options.hold_radius;
    let near = |x: u32, y: u32| (x as f32 - 256.).hypot(y as f32 - 160.) < options.hold_radius;
    assert!(
        largest_difference(&inhaled, &plain, far) <= 2,
        "the hold reached far from the point"
    );
    assert!(
        largest_difference(&inhaled, &plain, near) > 100,
        "nothing held at the point"
    );
    assert!(
        largest_difference(&inhaled, &exhaled, near) > 100,
        "the hold does not breathe"
    );
}
