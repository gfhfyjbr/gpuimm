use super::{
    GlimmController, GlimmDebugView, GlimmDemo, GlimmDirection, GlimmEasing, GlimmEffect,
    GlimmOptions, GlimmPalette, GlimmPaletteName, GlimmSample, GlimmShaderSource, GlimmTimeline,
    GlimmUniforms, ScopePaint, glimm_origin, glimm_scope, glimm_shader_error, uniforms,
};
use crate::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Scene, Styled as _,
    TestAppContext, Window, div, point, px, size,
};
use std::rc::Rc;

fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}

#[test]
fn palettes_match_the_published_javascript() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("reference.json")).unwrap();
    for name in GlimmPaletteName::ALL {
        let actual = name.palette();
        let expected: GlimmPalette =
            serde_json::from_value(fixture["palettes"][name.name()].clone()).unwrap();
        for (a, b) in [actual.a, actual.b, actual.c, actual.d]
            .into_iter()
            .zip([expected.a, expected.b, expected.c, expected.d])
        {
            for (a, b) in a.into_iter().zip(b) {
                close(a, b);
            }
        }
    }
    let custom = GlimmPalette::from_colors(&[0x123456, 0xfedcba, 0xff0044]);
    let expected: GlimmPalette = serde_json::from_value(fixture["custom"].clone()).unwrap();
    for t in [0., 0.1, 0.5, 0.9, 1.] {
        for (a, b) in custom.sample(t).into_iter().zip(expected.sample(t)) {
            close(a, b);
        }
    }
}
#[test]
fn easings_match_the_published_javascript() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("reference.json")).unwrap();
    for easing in GlimmEasing::ALL {
        let name = serde_json::to_value(easing)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        for (index, t) in [0., 0.1, 0.25, 0.5, 0.75, 0.9, 1.].into_iter().enumerate() {
            close(
                easing.sample(t),
                fixture["easings"][&name][index].as_f64().unwrap() as f32,
            );
        }
    }
}
#[test]
fn single_anchor_is_constant_and_empty_chain_has_a_defined_fallback() {
    let p = GlimmPalette::from_colors(&[0xff0000]);
    assert_eq!(p.sample(0.5), [1., 0., 0.]);
    assert_eq!(
        GlimmPalette::from_colors(&[]),
        GlimmPalette::from_colors(&[0x7B4FFF, 0x1FC8FF])
    );
}
#[test]
fn immediate_scope_uses_reference_release_and_traversal() {
    let timeline = GlimmTimeline::new(GlimmOptions::default(), false);
    let first = timeline.sample(0.);
    close(first.progress, 0.08);
    close(first.strength, 0.);
    let middle = timeline.sample(800.);
    close(middle.progress, 0.08 + 0.92 * (1. - 0.5f32.powf(2.4)));
    close(middle.strength, 0.5f32.powf(1.2));
    assert!(middle.midpoint);
    assert!(!timeline.sample(1600.).active);
}
#[test]
fn async_gather_holds_sharp_then_snaps_and_releases() {
    let mut timeline = GlimmTimeline::new(GlimmOptions::default(), true);
    let held = timeline.sample(3000.);
    close(held.gather, 0.32);
    close(held.haze, 0.);
    close(held.strength, 0.55);
    close(held.progress, 0.08);
    assert!(!held.burst);
    assert!(timeline.resolve(3000.));
    assert!(!timeline.resolve(3100.));
    let snap = 440. * (1. - 0.32);
    let start = timeline.sample(3000. + snap);
    close(start.gather, 1.);
    assert!(start.burst);
    assert!(timeline.sample(3000. + snap + 220.).gather < 0.0001);
    assert!(!timeline.sample(3000. + snap + 1600.).active);
}
#[test]
fn anticipation_without_work_reaches_full_pull() {
    let mut timeline = GlimmTimeline::anticipation(GlimmOptions::default());
    close(timeline.sample(620.).gather, 1.);
    assert!(timeline.resolve(620.));
    assert!(timeline.sample(621.).burst);
    assert!(!timeline.sample(709.).midpoint);
    assert!(timeline.sample(710.).midpoint);
    close(
        GlimmTimeline::new(GlimmOptions::default(), true)
            .sample(620.)
            .gather,
        0.32,
    );
}
#[test]
fn element_coordinates_are_local_to_the_scope() {
    let controller = GlimmController::new();
    controller.0.borrow_mut().bounds =
        Bounds::new(point(px(160.), px(316.)), size(px(634.), px(360.)));
    assert_eq!(
        controller.local_point(point(px(477.), px(609.))),
        [317., 293.]
    );
    assert_eq!(GlimmDemo::Generate.options().full_bleed, Some(false));
    assert_eq!(GlimmDemo::Tasks.options().full_bleed, Some(true));
}
#[test]
fn early_results_do_not_wait_for_anticipation() {
    let mut timeline = GlimmTimeline::new(GlimmOptions::default(), true);
    assert!(timeline.resolve(50.));
    assert!(timeline.sample(51.).progress >= 0.08);
}
#[test]
fn cancellation_and_fast_forward_settle() {
    let mut timeline = GlimmTimeline::new(GlimmOptions::default(), true);
    timeline.cancel(620.);
    assert!(!timeline.resolve(700.));
    close(timeline.sample(780.).gather, 0.08);
    assert!(!timeline.sample(940.).active);
    let mut timeline = GlimmTimeline::new(GlimmOptions::default(), false);
    let before = timeline.sample(300.);
    timeline.fast_forward(300.);
    close(timeline.sample(300.).progress, before.progress);
    assert!(!timeline.sample(460.).active);
}
#[test]
fn fast_forward_while_waiting_still_reveals_the_committed_result() {
    let mut timeline = GlimmTimeline::new(GlimmOptions::default(), true);
    timeline.fast_forward(200.);
    assert!(timeline.resolve(2000.));
    assert!(timeline.sample(2000.).active);
    assert!(!timeline.sample(2160.).active);
    let mut timeline = GlimmTimeline::new(GlimmOptions::band(), false);
    timeline.fast_forward(200.);
    assert!(!timeline.sample(580.).active);
}
#[test]
fn replay_preserves_scope_identity_and_rebases_its_snapshot_range() {
    let controller = GlimmController::new();
    let b = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.))).scale(1.);
    let quad = crate::Quad {
        bounds: b,
        content_mask: crate::ContentMask { bounds: b },
        background: crate::rgb(0xffffff).into(),
        ..Default::default()
    };
    let mut source = Scene::default();
    source.insert_primitive(quad);
    source.fast_glimm.scopes.push(ScopePaint {
        controller: Rc::downgrade(&controller.0),
        paint_range: 0..1,
    });
    let mut copy = Scene::default();
    copy.insert_primitive(quad);
    copy.replay(0..1, &source);
    assert_eq!(copy.fast_glimm.scopes[0].paint_range, 1..2);
    assert!(
        copy.fast_glimm.scopes[0]
            .controller
            .ptr_eq(&Rc::downgrade(&controller.0))
    );
    copy.clear();
    assert!(copy.fast_glimm.scopes.is_empty());
    copy.replay_balanced(0..1, &source).unwrap();
    assert_eq!(copy.fast_glimm.scopes[0].paint_range, 0..1);
}
#[test]
fn fades_end_the_scope_before_it_leaves_the_box() {
    let timeline = GlimmTimeline::new(GlimmDemo::Invoice.options(), false);
    assert!(!timeline.sample(1600.).active);
}
#[test]
fn band_defaults_keep_the_original_centred_cross_axis_and_margin() {
    let options = GlimmOptions::band();
    let u = uniforms(
        &options,
        GlimmSample::default(),
        Bounds::new(point(px(0.), px(0.)), size(px(600.), px(400.))),
        1.,
        false,
        0.,
    );
    close(u.0[5][3], 0.5);
    close(u.0[8][0], -0.2);
    close(u.0[8][1], 1.2);
    close(u.0[7][2], 0.);
}
#[test]
fn invalid_user_inputs_cannot_poison_gpu_uniforms() {
    let options = GlimmOptions {
        sweep_ms: f32::NAN,
        band_tight: 0.,
        span: -10.,
        curve: f32::INFINITY,
        at: Some([f32::NAN, 12.]),
        fade: Some([0.7, 0.2]),
        ..GlimmOptions::default()
    }
    .validated();
    assert_eq!(options.sweep_ms, 1600.);
    assert_eq!(options.band_tight, 0.1);
    assert_eq!(options.span, 0.05);
    assert!(options.fade.is_none());
    let u = uniforms(
        &options,
        GlimmSample::default(),
        Bounds::new(point(px(0.), px(0.)), size(px(600.), px(400.))),
        2.,
        false,
        0.,
    );
    assert!(u.0.into_iter().flatten().all(f32::is_finite));
    assert_eq!(std::mem::size_of::<GlimmUniforms>(), 18 * 16);
}
#[test]
fn presets_round_trip_as_customisable_json() {
    for demo in GlimmDemo::ALL {
        let options = demo.options();
        assert_eq!(
            options,
            serde_json::from_str::<GlimmOptions>(&serde_json::to_string(&options).unwrap())
                .unwrap()
        );
    }
    assert_eq!(GlimmDemo::Teammate.options().span, 0.64);
    assert_eq!(GlimmDemo::Teammate.options().intensity, 1.);
    assert_eq!(GlimmDemo::Tasks.options().curve, 0.12);
    assert_eq!(GlimmDemo::Tasks.options().fade, None);
    assert_eq!(GlimmDemo::Generate.options().direction, GlimmDirection::Btt);
}

struct TestScope {
    controller: GlimmController,
}
impl Render for TestScope {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        glimm_scope(
            "scope",
            &self.controller,
            div().size_full().child("native text"),
        )
    }
}
#[test]
fn cancelling_or_fast_forwarding_a_seeked_run_resumes_from_the_seek_time() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            let token = view
                .controller
                .anticipate(GlimmOptions::default(), window, cx);
            view.controller.seek(620., window);
            view.controller.cancel(token, window, cx);
            {
                let state = view.controller.0.borrow();
                let run = state.run.as_ref().unwrap();
                assert!(run.paused.is_none());
                assert!((super::elapsed(run, cx) - 620.).abs() < 1.);
            }
            view.controller.play(GlimmOptions::default(), window, cx);
            view.controller.seek(300., window);
            view.controller.fast_forward(window, cx);
            let state = view.controller.0.borrow();
            let run = state.run.as_ref().unwrap();
            assert!(run.paused.is_none());
            assert!((super::elapsed(run, cx) - 300.).abs() < 1.);
        })
        .unwrap();
}
#[test]
fn prepared_snapshot_survives_entrance_and_resolve() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            view.controller.prepare(window);
            let old = view.controller.0.borrow().prepared.clone().unwrap();
            let token = view
                .controller
                .anticipate_change(GlimmOptions::default(), window, cx);
            assert!(Rc::ptr_eq(
                &view.controller.0.borrow().run.as_ref().unwrap().old,
                &old
            ));
            view.controller.seek(620., window);
            assert!(view.controller.resolve(token, window, cx));
            assert!(Rc::ptr_eq(
                &view.controller.0.borrow().run.as_ref().unwrap().old,
                &old
            ));
            view.controller.stop(window);
            assert!(view.controller.0.borrow().prepared.is_none());
        })
        .unwrap();
}
#[test]
fn reduced_motion_still_commits_work_and_rejects_stale_results() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            cx.set_reduce_motion(true);
            let old = view
                .controller
                .anticipate(GlimmOptions::default(), window, cx);
            let new = view
                .controller
                .anticipate(GlimmOptions::default(), window, cx);
            assert!(!view.controller.resolve(old, window, cx));
            assert!(view.controller.resolve(new, window, cx));
            assert!(!view.controller.resolve(new, window, cx));
            assert!(!view.controller.sample(cx).active);
        })
        .unwrap();
}
#[test]
fn scope_keeps_layout_and_does_not_schedule_idle_frames() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        assert!(window.rendered_frame.scene.fast_glimm.frames.is_empty());
        assert!(window.next_frame_callbacks.borrow().is_empty());
    })
    .unwrap();
}
#[test]
fn a_run_knows_its_end_once_it_can_finish() {
    let mut waiting = GlimmTimeline::new(GlimmOptions::default(), true);
    assert_eq!(waiting.end_ms(), None);
    waiting.cancel(500.);
    assert_eq!(waiting.end_ms(), Some(820.));

    let options = GlimmOptions::band();
    let mut band = GlimmTimeline::new(options.clone(), false);
    assert_eq!(
        band.end_ms(),
        Some(options.sweep_ms.max(80.) + options.outro_ms)
    );
    band.fast_forward(100.);
    assert_eq!(band.end_ms(), Some(260. + options.outro_ms));

    let mut scope = GlimmTimeline::new(GlimmOptions::default(), false);
    let end = scope.end_ms().unwrap();
    assert!(scope.sample(end - 1.).active && !scope.sample(end).active);
    let mut longer = GlimmOptions::default();
    longer.sweep_ms = end + 900.;
    scope.set_options(longer);
    assert_eq!(scope.end_ms(), Some(end + 900.));
    assert!(scope.sample(end).active);
}
#[test]
fn transition_captures_before_the_change_and_resolve_with_guards_the_commit() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            let controller = view.controller.clone();
            let mut changed = false;
            let captured = controller.transition(GlimmOptions::default(), window, cx, |_, _| {
                changed = true;
                controller.0.borrow().run.is_some()
            });
            assert!(captured && changed);

            let stale = controller.anticipate(GlimmOptions::default(), window, cx);
            let fresh = controller.anticipate(GlimmOptions::default(), window, cx);
            assert_eq!(controller.resolve_with(stale, window, cx, |_, _| 1), None);
            assert_eq!(
                controller.resolve_with(fresh, window, cx, |_, _| 2),
                Some(2)
            );
            assert_eq!(controller.resolve_with(fresh, window, cx, |_, _| 3), None);
        })
        .unwrap();
}
#[test]
fn debug_view_and_retuned_settings_reach_a_paused_frame() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            assert_eq!(view.controller.elapsed_ms(cx), None);
            view.controller.play(GlimmOptions::default(), window, cx);
            view.controller.seek(300., window);
            assert_eq!(view.controller.debug_view(), GlimmDebugView::Off);
            view.controller.set_debug_view(GlimmDebugView::Blur, window);
            let mut options = GlimmOptions::default();
            options.sweep_ms = 3000.;
            options.span = 0.4;
            view.controller.retune(options, window);
            assert_eq!(view.controller.end_ms(), Some(3000.));
            assert_eq!(view.controller.elapsed_ms(cx), Some(300.));
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let uniforms = window.rendered_frame.scene.fast_glimm.frames[0].uniforms.0;
        assert_eq!(uniforms[9][3], 3.);
        assert_eq!(uniforms[5][2], 0.4);
    })
    .unwrap();
}
struct ClippedScope {
    controller: GlimmController,
}
impl Render for ClippedScope {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        glimm_scope(
            "scope",
            &self.controller,
            div().size_full().child(
                div()
                    .size(px(120.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .child("clipped text"),
            ),
        )
    }
}
#[test]
fn a_rounded_clip_in_the_frame_keeps_the_scope_and_its_run() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| ClippedScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            view.controller.play(GlimmOptions::default(), window, cx);
            view.controller.seek(300., window);
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let scene = &window.rendered_frame.scene;
        // Compiling rounded clips rebuilds the scene; what Glimm recorded has to come along.
        assert!(
            scene.fast_rounded_clip.compiled,
            "no rounded clip to compile"
        );
        assert_eq!(scene.fast_glimm.frames.len(), 1);
        assert_eq!(scene.fast_glimm.scopes.len(), 1);
        assert_eq!(
            scene.fast_glimm.frames[0].paint_range,
            scene.fast_glimm.scopes[0].paint_range
        );
        assert!(scene.fast_glimm.frames[0].paint_range.end <= scene.len());
    })
    .unwrap();
}
struct MarkedScope {
    controller: GlimmController,
}
impl Render for MarkedScope {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().pt(px(30.)).pl(px(10.)).child(glimm_scope(
            "scope",
            &self.controller,
            div().relative().size(px(200.)).child(glimm_origin(
                &self.controller,
                div()
                    .absolute()
                    .left(px(40.))
                    .top(px(20.))
                    .w(px(20.))
                    .h(px(10.)),
            )),
        ))
    }
}
#[test]
fn a_marked_element_gives_its_centre_in_scope_coordinates() {
    let mut cx = TestAppContext::single();
    let controller = GlimmController::new();
    assert_eq!(controller.marked_point(), None);
    let window = cx.add_window({
        let controller = controller.clone();
        |_, _| MarkedScope { controller }
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
    })
    .unwrap();
    assert_eq!(controller.marked_point(), Some([50., 25.]));
}
#[test]
fn a_followed_shader_is_taken_again_only_when_its_file_changes() {
    let mut built_in = GlimmShaderSource::following(None, "built in");
    assert_eq!(built_in.take_changed().as_deref(), Some("built in"));
    assert_eq!(built_in.take_changed(), None);

    let directory = std::env::temp_dir().join(format!("glimm-shader-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("glimm.metal");
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
    write("one", 1_000);
    let mut followed = GlimmShaderSource::following(Some(path.clone()), "built in");
    assert_eq!(followed.take_changed().as_deref(), Some("one"));
    assert_eq!(followed.take_changed(), None);
    write("two", 2_000);
    assert_eq!(followed.take_changed().as_deref(), Some("two"));

    // A missing file is reported once and leaves the caller on its current pipeline.
    std::fs::remove_file(&path).unwrap();
    assert_eq!(followed.take_changed(), None);
    assert!(glimm_shader_error().unwrap().contains("glimm.metal"));
    assert_eq!(followed.take_changed(), None);
    assert_eq!(followed.embedded(), "built in");
    write("three", 3_000);
    assert_eq!(followed.take_changed().as_deref(), Some("three"));
    std::fs::remove_dir_all(&directory).unwrap();
}

fn radial_presets() -> [GlimmOptions; 3] {
    [
        GlimmOptions::emerge(),
        GlimmOptions::shock(),
        GlimmOptions::collapse(),
    ]
}
fn scope(w: f32, h: f32) -> Bounds<crate::Pixels> {
    Bounds::new(point(px(0.), px(0.)), size(px(w), px(h)))
}
#[test]
fn radial_presets_are_within_their_ranges_and_round_trip_as_json() {
    for (options, effect) in radial_presets().into_iter().zip([
        GlimmEffect::Emerge,
        GlimmEffect::Shock,
        GlimmEffect::Collapse,
    ]) {
        assert_eq!(options.effect, effect);
        assert!(effect.is_radial());
        assert_eq!(
            options.clone().validated(),
            options,
            "{effect:?} is out of range"
        );
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            serde_json::from_str::<GlimmOptions>(&json).unwrap(),
            options
        );
    }
    assert!(!GlimmEffect::Scope.is_radial() && !GlimmEffect::Band.is_radial());
    // Old settings files, written before these fields existed, still load.
    let old: GlimmOptions = serde_json::from_str(r#"{"effect":"Shock","sweepMs":700}"#).unwrap();
    assert_eq!(old.sweep_ms, 700.);
    assert_eq!(old.amplitude, GlimmOptions::default().amplitude);
}
#[test]
fn radial_settings_are_clamped_before_they_reach_the_gpu() {
    let wild = GlimmOptions {
        amplitude: f32::INFINITY,
        ring_width: 0.,
        wavelength: -5.,
        ripples: 100.,
        echo: f32::NAN,
        falloff: 0.,
        chroma: 9.,
        tint: -1.,
        stretch_blur: 7.,
        squeeze: 2.,
        push: 1.,
        hold_amount: f32::NEG_INFINITY,
        hold_radius: 0.,
        breath_ms: 0.,
        band_overlay: 3.,
        ..GlimmOptions::emerge()
    }
    .validated();
    let defaults = GlimmOptions::default();
    assert_eq!(wild.amplitude, defaults.amplitude);
    assert_eq!(wild.ring_width, 4.);
    assert_eq!(wild.wavelength, 8.);
    assert_eq!(wild.ripples, 6.);
    assert_eq!(wild.echo, 0.);
    assert_eq!(wild.falloff, 16.);
    assert_eq!(wild.chroma, 2.);
    assert_eq!(wild.tint, 0.);
    assert_eq!(wild.stretch_blur, 2.);
    assert_eq!(wild.squeeze, 1.);
    assert_eq!(wild.push, 0.9);
    assert_eq!(wild.hold_amount, defaults.hold_amount);
    assert_eq!(wild.hold_radius, 8.);
    assert_eq!(wild.breath_ms, 200.);
    assert_eq!(wild.band_overlay, 1.5);
    for at in [[0., 0.], [-3000., 50.], [1e9, f32::NAN]] {
        let options = GlimmOptions {
            at: Some(at),
            intensity: 4.,
            ..wild.clone()
        }
        .validated();
        for t in [0., 300., 2000.] {
            let sample = GlimmTimeline::new(options.clone(), false).sample(t);
            let u = uniforms(&options, sample, scope(400., 300.), 2., false, 9.);
            assert!(
                u.0.into_iter().flatten().all(f32::is_finite),
                "{at:?} at {t}ms"
            );
        }
    }
}
#[test]
fn radial_timelines_run_the_front_out_spend_the_wave_and_end() {
    for options in radial_presets() {
        let effect = options.effect;
        let timeline = GlimmTimeline::new(options.clone(), false);
        let end = timeline.end_ms().unwrap();
        assert_eq!(end, options.sweep_ms, "{effect:?}");
        let start = timeline.sample(0.);
        assert!(start.active && start.burst, "{effect:?}");
        close(start.progress, 0.);
        close(start.strength, 0.);
        let mut last_progress = 0.;
        let mut peak = 0f32;
        for step in 1..20 {
            let t = end * step as f32 / 20.;
            let sample = timeline.sample(t);
            assert!(sample.active, "{effect:?} ended early at {t}");
            assert!(
                sample.progress >= last_progress,
                "{effect:?} front went back at {t}"
            );
            last_progress = sample.progress;
            if t > 200. {
                assert!(
                    sample.strength <= peak + 1e-6,
                    "{effect:?} gained energy at {t}"
                );
            }
            peak = peak.max(sample.strength);
        }
        assert!(peak > 0.5, "{effect:?} never had much energy: {peak}");
        let done = timeline.sample(end);
        assert!(!done.active, "{effect:?}");
        close(done.progress, 1.);
        close(done.strength, 0.);
        close(done.gather, 0.);
        // A pure function of time: seeking back gives the same instant.
        assert_eq!(timeline.sample(333.), timeline.sample(333.));
        assert!(timeline.sample(90.).midpoint && !timeline.sample(89.).midpoint);
    }
}
#[test]
fn a_held_radial_run_breathes_until_resolved_then_lets_go_into_the_effect() {
    let options = GlimmOptions::shock();
    let mut timeline = GlimmTimeline::new(options.clone(), true);
    assert_eq!(timeline.end_ms(), None);
    close(timeline.sample(options.gather_ms).gather, 1.);
    let held = timeline.sample(5000.);
    assert!(held.active && !held.burst);
    close(held.gather, 1.);
    close(held.progress, 0.);
    close(held.breath, (5000. / options.breath_ms).fract());
    assert!(timeline.resolve(5000.));
    assert_eq!(timeline.end_ms(), Some(5000. + options.sweep_ms));
    // No snap: the front starts at once, while the hold lets go over 260ms.
    let released = timeline.sample(5000.);
    assert!(released.burst);
    close(released.gather, 1.);
    assert!(timeline.sample(5130.).gather < 1. && timeline.sample(5130.).gather > 0.);
    close(timeline.sample(5260.).gather, 0.);
    assert!(timeline.sample(5300.).progress > 0.);
    assert!(!timeline.sample(5000. + options.sweep_ms).active);

    let mut cancelled = GlimmTimeline::new(options, true);
    cancelled.cancel(1000.);
    assert!(!cancelled.resolve(1100.));
    assert!(cancelled.sample(1160.).gather < 1.);
    assert!(!cancelled.sample(1320.).active);

    let mut fast = GlimmTimeline::new(GlimmOptions::emerge(), false);
    fast.fast_forward(200.);
    assert_eq!(fast.end_ms(), Some(360.));
    assert!(!fast.sample(360.).active);
}
#[test]
fn radial_uniforms_place_the_front_around_the_point_and_leave_glimm_dev_rows_alone() {
    let bounds = scope(400., 300.);
    // The glimm.dev paths keep rows 13 to 17 empty, so their shaders are untouched.
    for options in [GlimmOptions::default(), GlimmOptions::band()] {
        let sample = GlimmTimeline::new(options.clone(), false).sample(400.);
        let u = uniforms(&options, sample, bounds, 1., false, 0.);
        assert!(u.0[13..].iter().flatten().all(|v| *v == 0.));
    }
    let options = GlimmOptions {
        at: Some([100., 150.]),
        ..GlimmOptions::emerge()
    };
    let sample = GlimmSample {
        progress: 0.5,
        strength: 0.8,
        gather: 0.25,
        breath: 0.75,
        active: true,
        burst: true,
        ..GlimmSample::default()
    };
    let u = uniforms(&options, sample, bounds, 2., false, 0.).0;
    let cover = 300f32.hypot(150.);
    let reach = cover + 2. * options.ring_width + options.ripples * options.wavelength;
    assert_eq!(u[2][0..2], [100., 150.]);
    assert_eq!(
        u[13],
        [1., reach * 0.5, options.ring_width, options.amplitude * 0.8]
    );
    assert_eq!(
        u[14],
        [
            options.wavelength,
            options.ripples * options.wavelength,
            options.falloff,
            0.
        ]
    );
    assert_eq!(
        u[15],
        [
            options.chroma,
            options.tint,
            options.stretch_blur,
            options.squeeze
        ]
    );
    assert_eq!(
        u[16],
        [
            0.25 * options.hold_amount,
            0.75,
            options.hold_radius,
            options.push
        ]
    );
    assert_eq!(u[17], [0.8, 0., 0.5, cover]);

    // From a point outside the scope the front starts just short of the scope.
    let outside = GlimmOptions {
        at: Some([-120., 150.]),
        ..GlimmOptions::emerge()
    };
    let at_start = GlimmSample::default();
    let u = uniforms(&outside, at_start, bounds, 1., false, 0.).0;
    close(u[13][1], 120. - outside.ring_width);

    // Collapse runs inward and pulls; Shock neither squeezes nor pushes, and has the echo.
    let collapse = GlimmOptions {
        at: Some([200., 150.]),
        ..GlimmOptions::collapse()
    };
    let u = uniforms(&collapse, at_start, bounds, 1., false, 0.).0;
    close(u[13][0], 3.);
    close(u[13][1], 200f32.hypot(150.) + collapse.ring_width);
    close(u[16][3], -collapse.push);
    let shock = GlimmOptions::shock();
    let u = uniforms(&shock, sample, bounds, 1., false, 0.).0;
    close(u[13][0], 2.);
    close(u[14][3], shock.echo);
    close(u[15][3], 0.);
    close(u[16][3], 0.);
    // Intensity scales the wave, the squeeze and the push, within their limits.
    let strong = GlimmOptions {
        intensity: 4.,
        ..GlimmOptions::emerge()
    };
    let u = uniforms(&strong, sample, bounds, 1., false, 0.).0;
    close(u[13][3], strong.amplitude * 4. * 0.8);
    close(u[15][3], 1.);
    close(u[16][3], 0.9);
}
#[test]
fn a_radial_run_reaches_the_frame_with_its_rows() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| TestScope {
        controller: GlimmController::new(),
    });
    window
        .update(&mut cx, |view, window, cx| {
            let options = GlimmOptions {
                at: Some([-30., 20.]),
                ..GlimmOptions::emerge()
            };
            view.controller.play(options, window, cx);
            view.controller.seek(400., window);
            assert_eq!(
                view.controller.end_ms(),
                Some(GlimmOptions::emerge().sweep_ms)
            );
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let u = window.rendered_frame.scene.fast_glimm.frames[0].uniforms.0;
        assert_eq!(u[13][0], 1.);
        assert!(u[13][1] > 0. && u[13][3] > 0.);
        assert_eq!(u[2][0..2], [-30., 20.]);
    })
    .unwrap();
}
