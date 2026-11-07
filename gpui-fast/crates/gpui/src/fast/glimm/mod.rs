//! Native port of the band and HTML-in-Canvas effects on glimm.dev.
//! Shader mathematics and timing originate in glimm 0.4.1, MIT licensed.

mod options;
mod palette;
mod timeline;

pub use options::{GlimmDemo, GlimmDirection, GlimmEasing, GlimmEffect, GlimmOptions};
pub use palette::{GlimmPalette, GlimmPaletteName};
pub use timeline::{GlimmSample, GlimmTimeline};

use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, ScaledPixels, Scene, Window,
};
use scheduler::Instant;
use std::{
    cell::RefCell,
    ops::Range,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_RUN: AtomicU64 = AtomicU64::new(1);

/// Identifies one transition; late asynchronous results cannot resolve a newer run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlimmToken(u64);

/// A clonable handle shared by a scope and its event handlers.
#[derive(Clone, Default)]
pub struct GlimmController(Rc<RefCell<Controller>>);
#[derive(Default)]
struct Controller {
    run: Option<Run>,
    pending: Option<GlimmToken>,
    bounds: Bounds<Pixels>,
    epoch: Option<Instant>,
    prepared: Option<Rc<Scene>>,
}
struct Run {
    token: GlimmToken,
    capture_id: u64,
    start: Instant,
    shader_offset: f32,
    timeline: GlimmTimeline,
    old: Rc<Scene>,
    paused: Option<f32>,
    midpoint_sent: bool,
    preserve_old: bool,
}

impl GlimmController {
    /// Create an idle scope controller. No GPU allocation happens here.
    pub fn new() -> Self {
        Self::default()
    }
    /// Capture content before a local entrance animation, preserving it for the next transition.
    /// This is the reference's `prepare()` / `fromOld` path used by the teammate example.
    pub fn prepare(&self, window: &Window) {
        self.0.borrow_mut().prepared = Some(snapshot(window, &self.0));
    }
    /// Convert a window-local element center to this scope's logical-pixel coordinates.
    pub fn local_point(&self, point: crate::Point<Pixels>) -> [f32; 2] {
        let local = point - self.bounds().origin;
        [local.x.into(), local.y.into()]
    }
    /// Capture the old frame and start a sweep. Change application state immediately after this call.
    pub fn play(&self, options: GlimmOptions, window: &mut Window, cx: &App) -> GlimmToken {
        self.start(options, false, false, window, cx)
    }
    /// Start gathering while work runs. Call `resolve` immediately before committing its result.
    pub fn anticipate(&self, options: GlimmOptions, window: &mut Window, cx: &App) -> GlimmToken {
        self.start(options, true, false, window, cx)
    }
    /// Gather fully for an anticipated change without a work promise.
    /// Resolve after `options.gather_ms`; work anticipation instead holds at the reference's 0.32 pull.
    pub fn anticipate_change(
        &self,
        options: GlimmOptions,
        window: &mut Window,
        cx: &App,
    ) -> GlimmToken {
        self.start(options, true, true, window, cx)
    }
    fn start(
        &self,
        options: GlimmOptions,
        waiting: bool,
        anticipation_only: bool,
        window: &mut Window,
        cx: &App,
    ) -> GlimmToken {
        let now = cx.background_executor().now();
        let token = GlimmToken(NEXT_RUN.fetch_add(1, Ordering::Relaxed));
        let mut state = self.0.borrow_mut();
        let epoch = *state.epoch.get_or_insert(now);
        let shader_offset = (now - epoch).as_secs_f32();
        state.pending = waiting.then_some(token);
        let prepared = state.prepared.take();
        let preserve_old = prepared.is_some();
        state.run = (!cx.reduce_motion()).then(|| Run {
            token,
            capture_id: token.0,
            start: now,
            shader_offset,
            timeline: if anticipation_only {
                GlimmTimeline::anticipation(options)
            } else {
                GlimmTimeline::new(options, waiting)
            },
            old: prepared.unwrap_or_else(|| snapshot(window, &self.0)),
            paused: None,
            midpoint_sent: false,
            preserve_old,
        });
        window.refresh();
        token
    }
    /// Resolve only the matching pending run. Returns false for a stale or cancelled result.
    /// Reduced motion skips the visual effect but still accepts the matching work result.
    pub fn resolve(&self, token: GlimmToken, window: &mut Window, cx: &App) -> bool {
        let mut state = self.0.borrow_mut();
        if state.pending != Some(token) {
            return false;
        }
        state.pending = None;
        let Some(run) = &mut state.run else {
            window.refresh();
            return true;
        };
        let t = elapsed(run, cx);
        if !run.timeline.resolve(t) {
            return false;
        }
        if !run.preserve_old {
            run.old = snapshot(window, &self.0);
            run.capture_id = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
        }
        window.refresh();
        true
    }
    /// Cancel a run, returning gathered content to rest without a success sweep.
    pub fn cancel(&self, token: GlimmToken, window: &mut Window, cx: &App) {
        let mut state = self.0.borrow_mut();
        if state.pending == Some(token) {
            state.pending = None;
        }
        if let Some(run) = &mut state.run {
            if run.token == token {
                let t = elapsed(run, cx);
                run.timeline.cancel(t);
                run.start = cx.background_executor().now() - Duration::from_secs_f32(t / 1000.);
                run.paused = None;
                window.refresh();
            }
        }
    }
    /// Accelerate the current traversal to its end.
    pub fn fast_forward(&self, window: &mut Window, cx: &App) {
        if let Some(run) = &mut self.0.borrow_mut().run {
            let t = elapsed(run, cx);
            run.timeline.fast_forward(t);
            run.start = cx.background_executor().now() - Duration::from_secs_f32(t / 1000.);
            run.paused = None;
            window.refresh();
        }
    }
    /// Pause at the current instant, preserving both traversal and shader time.
    pub fn pause(&self, window: &mut Window, cx: &App) {
        if let Some(run) = &mut self.0.borrow_mut().run {
            run.paused = Some(elapsed(run, cx));
            window.refresh();
        }
    }
    /// Freeze at an elapsed time for inspection or reproducible screenshots.
    pub fn seek(&self, elapsed_ms: f32, window: &mut Window) {
        if let Some(run) = &mut self.0.borrow_mut().run {
            run.paused = Some(options::finite(elapsed_ms, 0., 0., 3_600_000.));
            window.refresh();
        }
    }
    /// Resume a paused run, without jumping its progress.
    pub fn resume(&self, window: &mut Window, cx: &App) {
        if let Some(run) = &mut self.0.borrow_mut().run {
            if let Some(t) = run.paused.take() {
                run.start = cx.background_executor().now() - Duration::from_secs_f32(t / 1000.);
                window.refresh();
            }
        }
    }
    /// Remove the effect immediately. Application content and input remain untouched.
    pub fn stop(&self, window: &mut Window) {
        let mut state = self.0.borrow_mut();
        state.run = None;
        state.pending = None;
        state.prepared = None;
        window.refresh();
    }
    /// The currently sampled shader state, or the idle state.
    pub fn sample(&self, cx: &App) -> GlimmSample {
        self.0
            .borrow()
            .run
            .as_ref()
            .map_or(GlimmSample::default(), |r| {
                r.timeline.sample(elapsed(r, cx))
            })
    }
    /// Deliver the midpoint (band) or burst (scope) event at most once per run.
    pub fn take_midpoint(&self, cx: &App) -> bool {
        let mut state = self.0.borrow_mut();
        let Some(run) = &mut state.run else {
            return false;
        };
        if !run.midpoint_sent && run.timeline.sample(elapsed(run, cx)).midpoint {
            run.midpoint_sent = true;
            true
        } else {
            false
        }
    }
    /// Last laid-out bounds, useful for converting a window-local click into `options.at`.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }
}
fn elapsed(run: &Run, cx: &App) -> f32 {
    run.paused
        .unwrap_or_else(|| (cx.background_executor().now() - run.start).as_secs_f32() * 1000.)
}
fn snapshot(window: &Window, controller: &Rc<RefCell<Controller>>) -> Rc<Scene> {
    let source = &window.rendered_frame.scene;
    let mut copy = Scene::default();
    let weak = Rc::downgrade(controller);
    let range = source
        .fast_glimm
        .scopes
        .iter()
        .find(|p| p.controller.ptr_eq(&weak))
        .map_or(0..0, |p| p.paint_range.clone());
    if let Err(error) = copy.replay_balanced(range, source) {
        log::error!("glimm snapshot: {error:#}");
    }
    copy.fast_glimm.frames.clear();
    copy.finish();
    Rc::new(copy)
}

/// Wrap arbitrary GPUI content. Layout, focus, hit testing and accessibility stay native.
pub fn glimm_scope(
    id: impl Into<ElementId>,
    controller: &GlimmController,
    child: impl IntoElement,
) -> GlimmScope {
    GlimmScope {
        id: id.into(),
        controller: controller.clone(),
        child: Some(child.into_any_element()),
    }
}
/// Layout-transparent GPUI scope element. Its visual effect is applied by the renderer.
pub struct GlimmScope {
    id: ElementId,
    controller: GlimmController,
    child: Option<AnyElement>,
}
impl IntoElement for GlimmScope {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for GlimmScope {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self
            .child
            .take()
            .expect("glimm scope layout requested twice");
        (child.request_layout(window, cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.controller.0.borrow_mut().bounds = bounds;
        child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let start = window.next_frame.scene.len();
        child.paint(window, cx);
        let range = start..window.next_frame.scene.len();
        window.next_frame.scene.fast_glimm.scopes.push(ScopePaint {
            controller: Rc::downgrade(&self.controller.0),
            paint_range: range.clone(),
        });
        let mut state = self.controller.0.borrow_mut();
        if cx.reduce_motion() {
            state.run = None;
            return;
        }
        let Some(run) = &state.run else {
            return;
        };
        let sample = run.timeline.sample(elapsed(run, cx));
        if !sample.active {
            state.run = None;
            return;
        }
        let clip = bounds.intersect(&window.content_mask().bounds);
        if bounds.size.width <= Pixels::ZERO
            || bounds.size.height <= Pixels::ZERO
            || clip.is_empty()
        {
            return;
        }
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let full = run.timeline.options().full_bleed.unwrap_or(
            bounds.size.width >= viewport.width - crate::px(1.)
                && bounds.size.height >= viewport.height - crate::px(1.),
        );
        let clock = run.shader_offset + elapsed(run, cx) / 1000.;
        let frame = GlimmFrame {
            run_id: run.capture_id,
            bounds: bounds.scale(scale),
            clip: clip.scale(scale),
            old_scene: run.old.clone(),
            paint_range: range,
            uniforms: uniforms(run.timeline.options(), sample, bounds, scale, full, clock),
        };
        window.next_frame.scene.fast_glimm.frames.push(frame);
        if run.paused.is_none() {
            window.request_animation_frame();
        }
    }
}

/// Renderer wire format. Every row matches a Metal `float4`; no implicit padding.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct GlimmUniforms(pub [[f32; 4]; 13]);
/// A scope's prepared paint operation, consumed by the platform renderer.
#[doc(hidden)]
#[derive(Clone)]
pub struct GlimmFrame {
    pub run_id: u64,
    pub bounds: Bounds<ScaledPixels>,
    pub clip: Bounds<ScaledPixels>,
    pub old_scene: Rc<Scene>,
    pub paint_range: Range<usize>,
    pub uniforms: GlimmUniforms,
}
/// Effects attached to a scene. Empty for ordinary GPUI applications.
#[doc(hidden)]
#[derive(Default)]
pub struct GlimmScene {
    pub frames: Vec<GlimmFrame>,
    scopes: Vec<ScopePaint>,
}
#[derive(Clone)]
struct ScopePaint {
    controller: std::rc::Weak<RefCell<Controller>>,
    paint_range: Range<usize>,
}

fn uniforms(
    o: &GlimmOptions,
    s: GlimmSample,
    bounds: Bounds<Pixels>,
    dpr: f32,
    full: bool,
    time: f32,
) -> GlimmUniforms {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let origin = o.at.unwrap_or([
        o.origin[0] * w + o.origin_offset[0],
        o.origin[1] * h + o.origin_offset[1],
    ]);
    let dir = o.direction.vector();
    let l = if o.direction.horizontal() { w } else { h };
    let center = if o.direction.horizontal() {
        origin[1] / h
    } else {
        origin[0] / w
    };
    let m = if full || o.effect == GlimmEffect::Band {
        0.2
    } else {
        (4.6 / o.band_tight).sqrt().max(0.2)
    };
    let reverse_gl = matches!(o.direction, GlimmDirection::Rtl | GlimmDirection::Ttb);
    let (pos_start, pos_end) = if reverse_gl {
        (1. + m, -m)
    } else {
        (-m, 1. + m)
    };
    let scale = (w.min(h) / 600.).clamp(0.25, 1.) * o.intensity;
    let feather = o
        .feather
        .unwrap_or_else(|| if full { 0. } else { 24f32.min(0.15 * w.min(h)) });
    GlimmUniforms([
        [w, h, dpr, time],
        [
            f32::from(bounds.origin.x) * dpr,
            f32::from(bounds.origin.y) * dpr,
            0.,
            0.,
        ],
        [origin[0], origin[1], dir[0], dir[1]],
        [s.progress, s.strength * o.peak_alpha, s.gather, s.haze],
        [
            s.ring,
            scale,
            if full { 1. } else { 0. },
            if s.burst { 1. } else { 0. },
        ],
        [o.band_tight, o.curve, o.span, center],
        [o.wave_amount, o.ripple_amount, o.wave_speed, o.brightness],
        [
            o.swell_amount,
            o.hue_shift,
            feather,
            if o.effect == GlimmEffect::Scope && o.page_effect {
                1.
            } else {
                0.
            },
        ],
        [
            pos_start,
            pos_end,
            (-m + (1. + 2. * m) * s.progress) * l,
            0.5 * l / o.band_tight.sqrt(),
        ],
        [o.palette.a[0], o.palette.a[1], o.palette.a[2], 0.],
        [o.palette.b[0], o.palette.b[1], o.palette.b[2], 0.],
        [o.palette.c[0], o.palette.c[1], o.palette.c[2], 0.],
        [o.palette.d[0], o.palette.d[1], o.palette.d[2], 0.],
    ])
}

pub(crate) fn clear(scene: &mut Scene) {
    scene.fast_glimm.frames.clear();
    scene.fast_glimm.scopes.clear();
}
pub(crate) fn replay(
    scene: &mut Scene,
    range: Range<usize>,
    previous: &Scene,
    destination_start: usize,
) {
    for scope in &previous.fast_glimm.scopes {
        if scope.paint_range.start >= range.start && scope.paint_range.end <= range.end {
            let mut scope = scope.clone();
            scope.paint_range = (scope.paint_range.start - range.start + destination_start)
                ..(scope.paint_range.end - range.start + destination_start);
            scene.fast_glimm.scopes.push(scope);
        }
    }
    for frame in &previous.fast_glimm.frames {
        if frame.paint_range.start >= range.start && frame.paint_range.end <= range.end {
            let mut frame = frame.clone();
            frame.paint_range = (frame.paint_range.start - range.start + destination_start)
                ..(frame.paint_range.end - range.start + destination_start);
            scene.fast_glimm.frames.push(frame);
        }
    }
}

#[cfg(test)]
mod tests;
