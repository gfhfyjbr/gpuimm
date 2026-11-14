//! The nine glimm.dev examples, rendered natively by GPUI and the Metal Glimm compositor.
#[path = "glimm_distort.rs"]
mod distort;
#[path = "glimm_motion.rs"]
mod motion;
#[path = "glimm_scenes.rs"]
mod scenes;

use gpui::accesskit::{Role, Toggled};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClipboardItem, Context, ElementId, FocusHandle,
    GlimmController, GlimmDebugView, GlimmDemo, GlimmDirection, GlimmEffect, GlimmOptions,
    GlimmPalette, GlimmPaletteName, GlimmToken, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowOptions, canvas, div, glimm_scope, point, px, rgb, size,
};
use scenes::{Control, HEIGHT, Layer, Scenes, WIDTH, route};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant, SystemTime},
};

struct Playground {
    focus: FocusHandle,
    entity: gpui::WeakEntity<Playground>,
    controller: GlimmController,
    scenes: Scenes,
    demo: GlimmDemo,
    options: GlimmOptions,
    palette: Option<GlimmPaletteName>,
    changed: bool,
    sheet_button_filled: bool,
    upgrade_open: bool,
    target: bool,
    busy: bool,
    pending: Option<GlimmToken>,
    running: bool,
    overlay: bool,
    overlay_at: Option<Instant>,
    face_at: Option<Instant>,
    inspect_burst_age: Option<f32>,
    inspect_leave_age: Option<f32>,
    tasks: u8,
    task_origin: u8,
    seconds: u8,
    epoch: u64,
    status: SharedString,
    paused: bool,
    element_origin: bool,
    last_at: Option<[f32; 2]>,
    stage: Rc<RefCell<Bounds<Pixels>>>,
    scene_only: bool,
    inspect: bool,
    debug_anchor: bool,
    config_path: Option<PathBuf>,
    config_stamp: Option<SystemTime>,
    config_error: Option<SharedString>,
    shader_stamp: Option<SystemTime>,
    shader_error: Option<String>,
    frame_at: Option<Instant>,
    frame_ms: f32,
}
/// Overlay the keys of a partial JSON settings file on `base`.
fn merge_config(base: &GlimmOptions, text: &str) -> Result<GlimmOptions, String> {
    let config: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let config = config.as_object().ok_or("settings must be a JSON object")?;
    let mut merged = serde_json::to_value(base).map_err(|e| e.to_string())?;
    for (key, value) in config {
        merged[key] = value.clone();
    }
    serde_json::from_value::<GlimmOptions>(merged)
        .map(GlimmOptions::validated)
        .map_err(|e| e.to_string())
}
fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|data| data.modified())
        .ok()
}
#[derive(Clone, Copy)]
enum Event {
    Auto,
    ReopenUpgrade,
    Commit(GlimmToken),
    Countdown(u8),
    Replay,
    HideToast,
}
impl Playground {
    fn new(demo: GlimmDemo, scene_only: bool, inspect: bool, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            entity: cx.weak_entity(),
            controller: GlimmController::new(),
            scenes: Scenes::load(cx),
            demo,
            options: demo.options(),
            palette: None,
            changed: false,
            sheet_button_filled: false,
            upgrade_open: true,
            target: true,
            busy: false,
            pending: None,
            running: false,
            overlay: false,
            overlay_at: None,
            face_at: None,
            inspect_burst_age: None,
            inspect_leave_age: None,
            tasks: 3,
            task_origin: 5,
            seconds: 3,
            epoch: 0,
            status: "Original scene · native GPUI".into(),
            paused: false,
            element_origin: true,
            last_at: None,
            stage: Rc::new(RefCell::new(Bounds::default())),
            scene_only,
            inspect,
            debug_anchor: false,
            config_path: None,
            config_stamp: None,
            config_error: None,
            shader_stamp: gpui::glimm_shader_stamp(),
            shader_error: None,
            frame_at: None,
            frame_ms: 0.,
        }
    }
    /// Read the settings file and apply it to the panel and to a run in flight.
    fn load_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        self.config_stamp = modified(&path);
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| merge_config(&self.options, &text));
        match loaded {
            Ok(options) => {
                self.element_origin = options.at.is_none();
                self.options = options;
                self.config_error = None;
                let options = self.run_options();
                self.controller.retune(options, window);
            }
            Err(error) => {
                let problem = format!("{}: {error}", path.display());
                eprintln!("glimm settings not applied: {problem}");
                self.config_error = Some(problem.into());
            }
        }
        cx.notify();
    }
    /// Pick up an edited settings file or shader. Returns whether anything is being followed.
    fn follow_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let shader = gpui::glimm_shader_stamp();
        if shader != self.shader_stamp {
            self.shader_stamp = shader;
            // The renderer rebuilds its pipeline when it next draws an effect frame.
            window.refresh();
            cx.notify();
        }
        if let Some(path) = &self.config_path {
            if modified(path) != self.config_stamp {
                self.load_config(window, cx);
            }
        }
        // The renderer learns of a bad shader while drawing; say so where the editor is too.
        let error = gpui::glimm_shader_error();
        if error != self.shader_error {
            match &error {
                Some(error) => eprintln!("glimm shader not applied: {error}"),
                None => eprintln!("glimm shader applied"),
            }
            self.shader_error = error;
            cx.notify();
        }
        self.config_path.is_some() || std::env::var_os(gpui::GLIMM_SHADER_DIR_ENV).is_some()
    }
    /// Smooth the interval between redraws of a playing effect; a paused or idle one has none.
    fn measure_frame(&mut self, playing: bool) {
        let now = Instant::now();
        match (playing, self.frame_at.replace(now)) {
            (true, Some(last)) => {
                let ms = (now - last).as_secs_f32() * 1000.;
                self.frame_ms = if self.frame_ms > 0. {
                    self.frame_ms * 0.9 + ms * 0.1
                } else {
                    ms
                };
            }
            (true, None) => {}
            (false, _) => {
                self.frame_at = None;
                self.frame_ms = 0.;
            }
        }
    }
    /// What one active run holds on the GPU, estimated from the renderers' allocation scheme:
    /// a viewport-sized raster target and output, plus the old and current scope crops with
    /// their mipmaps, at four bytes a pixel.
    fn texture_megabytes(&self, window: &Window) -> f32 {
        let pixels = |size: gpui::Size<Pixels>| {
            f32::from(size.width) * f32::from(size.height) * window.scale_factor().powi(2)
        };
        let viewport = pixels(window.viewport_size());
        let scope = pixels(self.controller.bounds().size);
        (2. * viewport + 2. * scope * 4. / 3.) * 4. / (1024. * 1024.)
    }
    /// Freeze the run at `fraction` of its length, short of the end, which would finish it.
    fn scrub(&mut self, fraction: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(length) = self.run_length(cx) else {
            return;
        };
        self.controller
            .seek((fraction.clamp(0., 1.) * length).min(length - 1.), window);
        self.paused = true;
        cx.notify();
    }
    /// The length the scrubber spans: the run's end once known, else what has gathered so far.
    fn run_length(&self, cx: &App) -> Option<f32> {
        let elapsed = self.controller.elapsed_ms(cx)?;
        Some(
            self.controller
                .end_ms()
                .unwrap_or_else(|| elapsed.max(self.options.gather_ms))
                .max(1.),
        )
    }
    fn scrubber(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = self.controller.elapsed_ms(cx);
        let length = self.run_length(cx);
        let fraction = elapsed
            .zip(length)
            .map_or(0., |(t, length)| (t / length).clamp(0., 1.));
        let bounds = Rc::new(RefCell::new(Bounds::<Pixels>::default()));
        let paint_bounds = bounds.clone();
        let click_bounds = bounds.clone();
        let at = move |bounds: &RefCell<Bounds<Pixels>>, x: Pixels| {
            let b = *bounds.borrow();
            ((x - b.origin.x) / b.size.width).clamp(0., 1.)
        };
        let label = match elapsed.zip(length) {
            Some((t, length)) => format!("{t:.0} / {length:.0} ms"),
            None => "no run · start an action, then scrub".to_owned(),
        };
        div()
            .w(px(WIDTH))
            .flex()
            .items_center()
            .gap_3()
            .text_size(px(12.))
            .text_color(rgb(0x737373))
            .opacity(if elapsed.is_some() { 1. } else { 0.5 })
            .child(
                div()
                    .id("scrubber")
                    .focusable()
                    .tab_stop(true)
                    .role(Role::Slider)
                    .aria_label("Effect time")
                    .relative()
                    .flex_1()
                    .h(px(18.))
                    .cursor_pointer()
                    .focus_visible(|s| s.bg(rgb(0xe5efff)))
                    .child(
                        canvas(
                            move |b, _, _| *paint_bounds.borrow_mut() = b,
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(8.))
                            .w_full()
                            .h(px(2.))
                            .bg(rgb(0xe4e4e7)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(gpui::relative(fraction))
                            .top(px(4.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(rgb(0x27272a)),
                    )
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            this.scrub(at(&click_bounds, event.position().x), window, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(
                        move |this, event: &gpui::MouseMoveEvent, window, cx| {
                            if event.dragging() {
                                this.scrub(at(&bounds, event.position.x), window, cx);
                            }
                        },
                    ))
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        // One 60 Hz frame a press; a tenth of a second with Shift.
                        let step = if event.keystroke.modifiers.shift {
                            100.
                        } else {
                            1000. / 60.
                        };
                        let step = match event.keystroke.key.as_str() {
                            "left" | "down" => -step,
                            "right" | "up" => step,
                            _ => return,
                        };
                        if let Some((t, length)) =
                            this.controller.elapsed_ms(cx).zip(this.run_length(cx))
                        {
                            this.scrub((t + step) / length, window, cx);
                        }
                        cx.stop_propagation();
                    })),
            )
            .child(div().w(px(150.)).flex_shrink_0().child(label))
    }
    fn later(&self, ms: u64, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        let epoch = self.epoch;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.epoch != epoch {
                    return;
                }
                match event {
                    Event::Auto => this.trigger(window, cx),
                    Event::ReopenUpgrade => {
                        this.upgrade_open = true;
                        cx.notify();
                    }
                    Event::Commit(token) => this.commit(token, window, cx),
                    Event::Replay => {
                        this.reset(window, cx);
                        this.trigger(window, cx);
                    }
                    Event::HideToast => {
                        this.overlay = false;
                        cx.notify();
                    }
                    Event::Countdown(seconds) => {
                        this.seconds = seconds;
                        if seconds == 0 {
                            this.trigger(window, cx);
                        } else {
                            this.later(1000, Event::Countdown(seconds - 1), window, cx);
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn start_automatic(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.inspect {
            return;
        }
        match self.demo {
            GlimmDemo::Teammate => self.later(1200, Event::Auto, window, cx),
            GlimmDemo::Invoice => self.later(900, Event::Auto, window, cx),
            GlimmDemo::UsageReset => self.later(1000, Event::Countdown(2), window, cx),
            _ => {}
        }
    }
    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.controller.stop(window);
        self.pending = None;
        self.running = false;
        self.busy = false;
        self.changed = false;
        self.sheet_button_filled = false;
        self.upgrade_open = true;
        self.target = true;
        self.overlay = false;
        self.overlay_at = None;
        self.face_at = None;
        self.inspect_burst_age = None;
        self.inspect_leave_age = None;
        self.tasks = 3;
        self.task_origin = 5;
        self.seconds = 3;
        self.paused = false;
        self.last_at = None;
        self.status = "Original scene · native GPUI".into();
        cx.notify();
    }
    fn select(&mut self, demo: GlimmDemo, window: &mut Window, cx: &mut Context<Self>) {
        self.reset(window, cx);
        self.demo = demo;
        self.options = demo.options();
        self.palette = None;
        self.element_origin = true;
        self.start_automatic(window, cx);
    }
    fn key(&self, final_state: bool) -> String {
        format!(
            "{}-{}",
            route(self.demo),
            if final_state { "final" } else { "initial" }
        )
    }
    fn scene_keys(&self) -> (String, String) {
        if self.demo == GlimmDemo::Upgrade && !self.upgrade_open {
            return ("upgrade-closed".into(), "upgrade-closed".into());
        }
        if self.demo == GlimmDemo::Tasks {
            let key = if self.tasks == 31 && !self.overlay {
                "tasks-complete".to_owned()
            } else {
                format!("tasks-{}", self.tasks)
            };
            return (key.clone(), key);
        }
        let inside = if self.busy
            && self.target
            && !self.changed
            && self
                .scenes
                .states
                .contains_key(&format!("{}-busy", route(self.demo)))
        {
            format!("{}-busy", route(self.demo))
        } else {
            self.key(self.changed)
        };
        let outside = match self.demo {
            GlimmDemo::Generate if self.busy => "sheet-busy".to_owned(),
            GlimmDemo::Generate => self.key(self.sheet_button_filled),
            GlimmDemo::Teammate if self.overlay => "join-final".into(),
            GlimmDemo::Teammate if self.changed => "join-arriving".into(),
            GlimmDemo::Teammate => "join-initial".into(),
            GlimmDemo::Invoice => self.key(self.overlay),
            _ => inside.clone(),
        };
        (inside, outside)
    }
    fn anchor(&self) -> Option<[f32; 4]> {
        let initial = self.scenes.get(&self.key(false));
        match self.demo {
            GlimmDemo::Tasks => initial
                .controls
                .iter()
                .find(|c| c.label == format!("Task {}", self.task_origin))
                .map(|c| c.bounds),
            GlimmDemo::Generate => initial.anchors.get("composer").copied(),
            GlimmDemo::UsageReset => initial.anchors.get("resetDock").copied().map(|mut b| {
                b[1] += b[3] - 40.;
                b[3] = 0.;
                b
            }),
            GlimmDemo::Teammate => initial
                .controls
                .iter()
                .find(|c| c.label == "Replay the join")
                .map(|c| c.bounds),
            GlimmDemo::Invoice | GlimmDemo::Upgrade => None,
            _ => {
                let label = match self.demo {
                    GlimmDemo::Upgrade => "Upgrade",
                    GlimmDemo::Publish => {
                        if self.changed {
                            "Back"
                        } else {
                            "Publish"
                        }
                    }
                    GlimmDemo::Autopilot => "Autopilot",
                    GlimmDemo::Theme => "Theme",
                    GlimmDemo::Tasks => "Task 5",
                    _ => "",
                };
                let current = self.scenes.get(&self.key(self.changed));
                current
                    .controls
                    .iter()
                    .find(|c| c.label == label)
                    .map(|c| c.bounds)
            }
        }
    }
    fn run_options(&mut self) -> GlimmOptions {
        let mut options = self.options.clone();
        if self.element_origin {
            options.at = self.anchor().map(|b| {
                let stage = self.stage.borrow();
                self.controller
                    .local_point(stage.origin + point(px(b[0] + b[2] / 2.), px(b[1] + b[3] / 2.)))
            });
        }
        if self.demo == GlimmDemo::Theme && self.changed && self.palette.is_none() {
            options.palette = GlimmPalette::from_colors(&[0x00C8B3, 0x00C0E8, 0x0088FF, 0x6155F5]);
        }
        let bounds = self.controller.bounds();
        self.last_at = Some(options.at.unwrap_or([
            f32::from(bounds.size.width) * options.origin[0] + options.origin_offset[0],
            f32::from(bounds.size.height) * options.origin[1] + options.origin_offset[1],
        ]));
        options
    }
    fn trigger(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            if self.demo == GlimmDemo::Generate {
                self.cancel(window, cx);
            }
            return;
        }
        self.target = !self.changed;
        match self.demo {
            GlimmDemo::Generate => {
                self.begin(if self.target { 2600 } else { 0 }, false, window, cx)
            }
            GlimmDemo::Teammate => self.begin(0, true, window, cx),
            GlimmDemo::Invoice => self.begin(0, true, window, cx),
            GlimmDemo::Upgrade => self.begin(500, false, window, cx),
            GlimmDemo::UsageReset => self.begin(0, true, window, cx),
            GlimmDemo::Publish => {
                self.begin(if self.target { 1400 } else { 0 }, !self.target, window, cx)
            }
            GlimmDemo::Autopilot | GlimmDemo::Theme => self.begin(0, true, window, cx),
            GlimmDemo::Tasks => {
                self.tasks = 31;
                self.target = true;
                self.begin(0, true, window, cx);
            }
        }
    }
    fn begin(
        &mut self,
        work_ms: u64,
        anticipated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let options = self.run_options();
        let delay = if anticipated && !cx.reduce_motion() {
            options.gather_ms as u64
        } else {
            work_ms
        };
        let token = if anticipated && delay > 0 {
            self.controller.anticipate_change(options, window, cx)
        } else if delay > 0 {
            self.controller.anticipate(options, window, cx)
        } else {
            self.controller.play(options, window, cx)
        };
        self.running = true;
        self.busy = self.demo != GlimmDemo::Generate || self.target;
        self.paused = false;
        self.status = "Gather → commit → burst → sweep".into();
        if delay > 0 {
            self.pending = Some(token);
            if !self.inspect {
                self.later(delay, Event::Commit(token), window, cx);
            }
        } else {
            self.pending = None;
            self.changed = self.target;
        }
        if cx.reduce_motion() && delay == 0 {
            self.burst(window, cx);
            self.running = false;
        }
        cx.notify();
    }
    fn commit(&mut self, token: GlimmToken, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending != Some(token) || !self.controller.resolve(token, window, cx) {
            return;
        }
        self.pending = None;
        if !matches!(self.demo, GlimmDemo::Teammate | GlimmDemo::Autopilot) {
            self.changed = self.target;
        }
        if self.demo == GlimmDemo::Tasks {
            self.overlay = true;
            self.overlay_at = Some(Instant::now());
            if !self.inspect {
                self.later(3200, Event::HideToast, window, cx);
            }
        }
        if cx.reduce_motion() {
            self.burst(window, cx);
            self.running = false;
        }
        cx.notify();
    }
    fn burst(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        if self.demo == GlimmDemo::Generate {
            self.sheet_button_filled = self.target;
        }
        if matches!(self.demo, GlimmDemo::Teammate | GlimmDemo::Autopilot) {
            self.changed = self.target;
        }
        if self.demo == GlimmDemo::Teammate {
            self.face_at = Some(Instant::now());
        }
        if self.demo == GlimmDemo::Invoice || (self.demo == GlimmDemo::Tasks && !self.overlay) {
            self.overlay = true;
            self.overlay_at = Some(Instant::now());
            if self.demo == GlimmDemo::Tasks && !self.inspect {
                self.later(3200, Event::HideToast, window, cx);
            }
        }
        self.status = "Burst committed · original scope and anchor".into();
    }
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(token) = self.pending.take() {
            self.controller.cancel(token, window, cx);
        } else {
            self.controller.stop(window);
        }
        self.epoch += 1;
        self.busy = false;
        self.running = false;
        self.paused = false;
        self.status = "Cancelled · pending work discarded".into();
        cx.notify();
    }
    fn action(&mut self, label: &str, window: &mut Window, cx: &mut Context<Self>) {
        match label {
            "Generate" | "Stop" | "Clear" | "Upgrade" | "Publish" | "Back" | "Autopilot"
            | "Theme" => self.trigger(window, cx),
            "Replay the join" => {
                self.reset(window, cx);
                self.trigger(window, cx);
            }
            "Dismiss" => {
                self.overlay = false;
                self.overlay_at = None;
                if self.demo == GlimmDemo::Teammate {
                    self.changed = false;
                }
                self.later(1500, Event::Replay, window, cx);
                cx.notify();
            }
            "Close" => {
                if !self.busy && self.upgrade_open {
                    self.upgrade_open = false;
                    self.later(1500, Event::ReopenUpgrade, window, cx);
                    cx.notify();
                }
            }
            label if label.starts_with("Task ") => {
                if self.busy {
                    return;
                }
                if let Ok(index) = label[5..].parse::<u8>() {
                    let bit = 1 << (index - 1);
                    if self.tasks | bit == 31 && self.tasks & bit == 0 {
                        self.task_origin = index;
                        self.trigger(window, cx);
                    } else {
                        self.tasks ^= bit;
                        self.overlay = false;
                        cx.notify();
                    }
                }
            }
            "Prompt" => {
                cx.write_to_clipboard(ClipboardItem::new_string("Fill this sheet with our top 12 open deals for Q4: account, region, owner, deal size, stage and expected close date.".into()));
                self.status = "Original read-only prompt copied".into();
                cx.notify();
            }
            "Follow" | "Say hi" | "Add" | "Message" | "Send" => {
                self.status = "Reference UI control · no connected service".into();
                cx.notify();
            }
            _ => {}
        }
    }
    fn control(&self, control: &Control, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let b = control.bounds;
        let label = control.label.clone();
        let key_label = label.clone();
        let disabled = control.disabled;
        let role = match control.role.as_str() {
            "switch" => Role::Switch,
            "checkbox" => Role::CheckBox,
            "textarea" => Role::TextInput,
            _ => Role::Button,
        };
        let mut button = div()
            .id(ElementId::Name(format!("scene-control-{ix}").into()))
            .absolute()
            .left(px(b[0]))
            .top(px(b[1]))
            .w(px(b[2]))
            .h(px(b[3]))
            .rounded(px(6.))
            .focusable()
            .tab_stop(!disabled)
            .role(role)
            .aria_label(control.label.clone())
            .aria_description(if disabled { "Disabled" } else { "" })
            .focus_visible(|s| s.border_2().border_color(rgb(0x007aff)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                if !disabled {
                    this.action(&label, window, cx);
                }
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if !disabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.action(&key_label, window, cx);
                        cx.stop_propagation();
                    }
                }),
            );
        if matches!(role, Role::Switch | Role::CheckBox) {
            button = button.aria_toggled(if control.checked {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        button.into_any_element()
    }
    fn preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let (inside_key, outside_key) = self.scene_keys();
        let inside = self.scenes.get(&inside_key);
        let mut outside = self.scenes.get(&outside_key);
        let age = |clock: Option<Instant>, frozen: Option<f32>| {
            if self.inspect {
                frozen.unwrap_or(2.)
            } else {
                clock.map_or(2., |at| at.elapsed().as_secs_f32())
            }
        };
        if !cx.reduce_motion() {
            if self.demo == GlimmDemo::Invoice && self.overlay {
                outside = motion::invoice(outside, age(self.overlay_at, self.inspect_burst_age));
            } else if self.demo == GlimmDemo::Teammate {
                if self.changed {
                    outside = motion::joined_faces(
                        &self.scenes.get("join-initial"),
                        outside,
                        age(self.face_at, self.inspect_burst_age),
                    );
                }
                if self.overlay {
                    outside =
                        motion::popover(outside, age(self.overlay_at, self.inspect_leave_age));
                }
            }
        }
        let scope = inside.scope;
        let stage = self.stage.clone();
        let fade = 1.;
        let mut preview = div()
            .relative()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(rgb(0xffffff))
            .child(
                canvas(
                    move |bounds, _, _| {
                        *stage.borrow_mut() = bounds;
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(div().absolute().inset_0().child(self.scenes.picture(
                if self.demo == GlimmDemo::Generate {
                    self.scenes.get("sheet-initial")
                } else {
                    inside.clone()
                },
                Layer::Before,
                1.,
                None,
            )))
            .child(
                div()
                    .absolute()
                    .left(px(scope[0]))
                    .top(px(scope[1]))
                    .w(px(scope[2]))
                    .h(px(scope[3]))
                    .child(glimm_scope(
                        "original-content",
                        &self.controller,
                        self.scenes.picture(inside, Layer::Scope, 1., None),
                    )),
            )
            .child(div().absolute().inset_0().child(self.scenes.picture(
                outside.clone(),
                Layer::After,
                fade,
                Some(self.seconds),
            )));
        for (ix, control) in outside.controls.iter().enumerate() {
            preview = preview.child(self.control(control, ix, cx));
        }
        if self.debug_anchor {
            if let Some(at) = self.last_at {
                preview = preview.child(
                    div()
                        .absolute()
                        .left(px(scope[0] + at[0] - 4.))
                        .top(px(scope[1] + at[1] - 4.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(rgb(0xff383c)),
                );
            }
        }
        preview.into_any_element()
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        active: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let action = Rc::new(action);
        let click = action.clone();
        let label = label.into();
        let click_entity = self.entity.clone();
        let key_entity = self.entity.clone();
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(label.clone())
            .px_3()
            .py_2()
            .rounded(px(6.))
            .border_1()
            .text_size(px(12.))
            .cursor_pointer()
            .bg(rgb(if active { 0x27272a } else { 0xffffff }))
            .text_color(rgb(if active { 0xffffff } else { 0x525252 }))
            .border_color(rgb(if active { 0x27272a } else { 0xe5e5e5 }))
            .focus_visible(|s| s.border_color(rgb(0x007aff)))
            .hover(|s| s.opacity(0.8))
            .on_click(move |_, window, cx| {
                let _ = click_entity.update(cx, |this, cx| click(this, window, cx));
            })
            .on_key_down(move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    let _ = key_entity.update(cx, |this, cx| action(this, window, cx));
                    cx.stop_propagation();
                }
            })
            .child(label)
    }
}

#[derive(Clone, Copy)]
enum Knob {
    Sweep,
    Gather,
    Intensity,
    Curve,
    Span,
    Tightness,
    Brightness,
    Swell,
}
impl Knob {
    const ALL: [Self; 8] = [
        Self::Sweep,
        Self::Gather,
        Self::Intensity,
        Self::Curve,
        Self::Span,
        Self::Tightness,
        Self::Brightness,
        Self::Swell,
    ];
    fn info(self, o: &GlimmOptions) -> (&'static str, f32, f32, f32) {
        match self {
            Self::Sweep => ("Sweep · ms", o.sweep_ms, 300., 5000.),
            Self::Gather => ("Gather · ms", o.gather_ms, 100., 1800.),
            Self::Intensity => ("Deformation", o.intensity, 0., 2.),
            Self::Curve => ("Curve", o.curve, -0.3, 0.5),
            Self::Span => ("Span", o.span, 0.05, 1.),
            Self::Tightness => ("Band tightness", o.band_tight, 3., 60.),
            Self::Brightness => ("Brightness", o.brightness, 0., 1.5),
            Self::Swell => ("Swell", o.swell_amount, 0., 1.),
        }
    }
    fn set(self, o: &mut GlimmOptions, value: f32) {
        match self {
            Self::Sweep => o.sweep_ms = value,
            Self::Gather => o.gather_ms = value,
            Self::Intensity => o.intensity = value,
            Self::Curve => o.curve = value,
            Self::Span => o.span = value,
            Self::Tightness => o.band_tight = value,
            Self::Brightness => o.brightness = value,
            Self::Swell => o.swell_amount = value,
        }
    }
}
impl Playground {
    fn slider(&self, knob: Knob, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, value, min, max) = knob.info(&self.options);
        let fraction = (value - min) / (max - min);
        let bounds = Rc::new(RefCell::new(Bounds::<Pixels>::default()));
        let paint_bounds = bounds.clone();
        let change = move |this: &mut Playground, delta: f32, cx: &mut Context<Playground>| {
            knob.set(&mut this.options, (value + delta).clamp(min, max));
            cx.notify();
        };
        let click = cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
            let b = *bounds.borrow();
            let f = ((event.position().x - b.origin.x) / b.size.width).clamp(0., 1.);
            knob.set(&mut this.options, min + f * (max - min));
            cx.notify();
        });
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(name)
                    .child(format!("{value:.2}")),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("knob-{name}").into()))
                    .focusable()
                    .tab_stop(true)
                    .role(Role::Slider)
                    .aria_label(name)
                    .relative()
                    .h(px(18.))
                    .cursor_pointer()
                    .focus_visible(|s| s.bg(rgb(0xe5efff)))
                    .child(
                        canvas(
                            move |b, _, _| *paint_bounds.borrow_mut() = b,
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(8.))
                            .w_full()
                            .h(px(2.))
                            .bg(rgb(0xe4e4e7)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(gpui::relative(fraction))
                            .top(px(4.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(rgb(0x27272a)),
                    )
                    .on_click(click)
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "left" | "down" => change(this, -(max - min) / 50., cx),
                            "right" | "up" => change(this, (max - min) / 50., cx),
                            _ => return,
                        }
                        cx.stop_propagation();
                    })),
            )
    }
    fn settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut controls = div()
            .id("controls")
            .w(px(274.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .text_size(px(12.))
            .text_color(rgb(0x525252));
        controls = controls.child("CUSTOMIZE").child(
            div()
                .flex()
                .gap_2()
                .child(self.button(
                    "scope",
                    "Content",
                    self.options.effect == GlimmEffect::Scope,
                    |s, _, cx| {
                        s.options.effect = GlimmEffect::Scope;
                        cx.notify();
                    },
                ))
                .child(self.button(
                    "band",
                    "Band",
                    self.options.effect == GlimmEffect::Band,
                    |s, _, cx| {
                        s.options.effect = GlimmEffect::Band;
                        cx.notify();
                    },
                )),
        );
        let mut directions = div().flex().gap_1();
        for (ix, direction) in GlimmDirection::ALL.into_iter().enumerate() {
            directions = directions.child(self.button(
                ElementId::Name(format!("direction-{ix}").into()),
                format!("{direction:?}"),
                self.options.direction == direction,
                move |s, _, cx| {
                    s.options.direction = direction;
                    cx.notify();
                },
            ));
        }
        controls = controls.child(directions);
        let mut palettes = div().flex().flex_wrap().gap_1().child(self.button(
            "original-palette",
            "Original",
            self.palette.is_none(),
            |s, _, cx| {
                s.palette = None;
                s.options.palette = s.demo.options().palette;
                cx.notify();
            },
        ));
        for palette in GlimmPaletteName::ALL {
            palettes = palettes.child(self.button(
                ElementId::Name(format!("palette-{palette:?}").into()),
                format!("{palette:?}"),
                self.palette == Some(palette),
                move |s, _, cx| {
                    s.palette = Some(palette);
                    s.options.palette = palette.palette();
                    cx.notify();
                },
            ));
        }
        controls = controls.child(palettes);
        let mut views = div().flex().flex_wrap().gap_1();
        for (ix, view) in GlimmDebugView::ALL.into_iter().enumerate() {
            views = views.child(self.button(
                ElementId::Name(format!("debug-view-{ix}").into()),
                view.name(),
                self.controller.debug_view() == view,
                move |s, window, cx| {
                    s.controller.set_debug_view(view, window);
                    cx.notify();
                },
            ));
        }
        controls = controls.child("DEBUG VIEW").child(views);
        for knob in Knob::ALL {
            controls = controls.child(self.slider(knob, cx));
        }
        controls
            .child(self.button(
                "element-origin",
                if self.element_origin {
                    "Origin: original element"
                } else {
                    "Origin: custom options"
                },
                self.element_origin,
                |s, _, cx| {
                    s.element_origin = !s.element_origin;
                    cx.notify();
                },
            ))
            .child(self.button(
                "debug-anchor",
                "Show point of action",
                self.debug_anchor,
                |s, _, cx| {
                    s.debug_anchor = !s.debug_anchor;
                    cx.notify();
                },
            ))
            .child(self.button(
                "reduced-motion",
                if cx.reduce_motion() {
                    "Reduced motion: on"
                } else {
                    "Reduced motion: off"
                },
                cx.reduce_motion(),
                |_, window, cx| {
                    let reduced = !cx.reduce_motion();
                    cx.set_reduce_motion(reduced);
                    window.refresh();
                },
            ))
            .child(self.button("copy-json", "Copy JSON", false, |s, _, cx| {
                let mut o = s.options.clone();
                o.at = s.last_at;
                cx.write_to_clipboard(ClipboardItem::new_string(
                    serde_json::to_string_pretty(&o).unwrap(),
                ));
                s.status = "Options copied".into();
                cx.notify();
            }))
            .child(self.button("defaults", "Reset options", false, |s, _, cx| {
                s.options = s.demo.options();
                s.palette = None;
                s.element_origin = true;
                cx.notify();
            }))
    }
}
impl Render for Playground {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.controller.take_midpoint(cx) {
            self.burst(window, cx);
        }
        let sample = self.controller.sample(cx);
        self.measure_frame(sample.active && !self.paused);
        if self.running && !sample.active && self.pending.is_none() {
            self.running = false;
            self.busy = false;
            if self.demo == GlimmDemo::Teammate {
                self.overlay = true;
                self.overlay_at = Some(Instant::now());
            }
        }
        if !self.inspect
            && !cx.reduce_motion()
            && (self
                .overlay_at
                .is_some_and(|at| at.elapsed() < Duration::from_millis(1000))
                || self
                    .face_at
                    .is_some_and(|at| at.elapsed() < Duration::from_millis(1200)))
        {
            window.request_animation_frame();
        }
        let root = div()
            .id("glimm-playground")
            .track_focus(&self.focus)
            .tab_group()
            .tab_index(0)
            .tab_stop(false)
            .size_full()
            .bg(rgb(0xfafafa))
            .font_family("Inter")
            .text_color(rgb(0x242529))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx)
                    } else {
                        window.focus_next(cx)
                    }
                    cx.stop_propagation();
                } else if event.keystroke.key == "escape" {
                    this.cancel(window, cx);
                    cx.stop_propagation();
                }
            }));
        if self.scene_only {
            return root.child(self.preview(cx)).into_any_element();
        }
        let mut tabs = div().flex().flex_wrap().gap_2();
        for (ix, demo) in GlimmDemo::ALL.into_iter().enumerate() {
            let label = match demo {
                GlimmDemo::Generate => "Generate data",
                GlimmDemo::Teammate => "Teammate joins",
                GlimmDemo::Invoice => "Invoice paid",
                GlimmDemo::Upgrade => "Upgrade to Pro",
                GlimmDemo::UsageReset => "Usage reset",
                GlimmDemo::Publish => "Publish",
                GlimmDemo::Autopilot => "Autopilot",
                GlimmDemo::Theme => "Theme",
                GlimmDemo::Tasks => "Tasks",
            };
            tabs = tabs.child(self.button(
                ElementId::Name(format!("demo-{ix}").into()),
                label,
                self.demo == demo,
                move |s, window, cx| s.select(demo, window, cx),
            ));
        }
        let scope = self.controller.bounds();
        let mut actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                self.button("play", "Run original action", false, |s, w, cx| {
                    s.trigger(w, cx)
                }),
            )
            .child(self.button("replay", "Replay scene", false, |s, w, cx| {
                s.reset(w, cx);
                s.start_automatic(w, cx);
            }))
            .child(self.button("cancel", "Cancel", false, |s, w, cx| s.cancel(w, cx)))
            .child(self.button(
                "pause",
                if self.paused { "Resume" } else { "Pause" },
                self.paused,
                |s, w, cx| {
                    if s.paused {
                        s.controller.resume(w, cx)
                    } else {
                        s.controller.pause(w, cx)
                    }
                    s.paused = !s.paused;
                    cx.notify();
                },
            ))
            .child(self.button("finish", "Fast forward", false, |s, w, cx| {
                s.controller.fast_forward(w, cx);
                s.paused = false;
                cx.notify();
            }));
        if !sample.active {
            actions = actions.opacity(0.9);
        }
        root.flex()
            .flex_col()
            .child(
                div()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_size(px(24.)).child("Glimm · native examples"))
                    .child(tabs),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .border_t_1()
                    .border_color(rgb(0xececec))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .p_5()
                            .gap_4()
                            .items_center()
                            .child(self.preview(cx))
                            .child(self.scrubber(cx))
                            .child(actions)
                            .child(
                                div()
                                    .w(px(WIDTH))
                                    .text_size(px(12.))
                                    .text_color(rgb(0x737373))
                                    .child(self.status.clone())
                                    .child(format!(
                                        " · Scope {:.0} × {:.0} · at {:?}",
                                        f32::from(scope.size.width),
                                        f32::from(scope.size.height),
                                        self.last_at
                                    ))
                                    .child(if self.frame_ms > 0. {
                                        format!(
                                            " · frame {:.1} ms · ≈{:.0} MB of textures while active",
                                            self.frame_ms,
                                            self.texture_megabytes(window)
                                        )
                                    } else {
                                        String::new()
                                    }),
                            )
                            .children(
                                gpui::glimm_shader_error()
                                    .map(|error| format!("Shader not applied · {error}"))
                                    .into_iter()
                                    .chain(
                                        self.config_error
                                            .as_ref()
                                            .map(|error| format!("Settings not applied · {error}")),
                                    )
                                    .map(|problem| {
                                        div()
                                            .id(ElementId::Name(problem.clone().into()))
                                            .w(px(WIDTH))
                                            .max_h(px(96.))
                                            .overflow_y_scroll()
                                            .p_2()
                                            .rounded(px(6.))
                                            .bg(rgb(0xfef2f2))
                                            .text_size(px(11.))
                                            .text_color(rgb(0xb91c1c))
                                            .child(problem)
                                    }),
                            ),
                    )
                    .child(self.settings(cx)),
            )
            .into_any_element()
    }
}

pub fn run() {
    let args = std::env::args().collect::<Vec<_>>();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|ix| args.get(ix + 1))
            .cloned()
    };
    if args.iter().any(|a| a == "--help") {
        println!(
            "Native glimm.dev examples\n--demo 0..8 --config file.json --scene-only\n--debug-view off|mix|displacement|blur|front\n--capture file.png --state initial|busy|final|transition --time milliseconds --gather\n--width px --height px\n\nRadial distortions on a grid, text and cards, in a hidden window when capturing:\n--distort emerge|shock|collapse|hold|spawn [--full] [--time ms | --strip frames]\n          [--capture file.png] [--debug-view view] [--config file.json] [--width px --height px]\n\nWhile the window is open, edits to the --config file apply at once, a run in flight included.\n{}=DIR compiles DIR/glimm.metal (.wgsl, .hlsl) instead of the built-in shader and\nagain on every save; a shader that fails to build is reported and the previous one stays.",
            gpui::GLIMM_SHADER_DIR_ENV
        );
        return;
    }
    if let Some(name) = value("--distort") {
        let demo = distort::Demo::parse(&name).expect("--distort emerge|shock|collapse|hold|spawn");
        distort::run(demo, &args);
        return;
    }
    let demo = GlimmDemo::ALL[value("--demo")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(8)];
    let capture = value("--capture").map(PathBuf::from);
    let inspect = capture.is_some();
    let scene_only = args.iter().any(|a| a == "--scene-only");
    let state = value("--state").unwrap_or_else(|| "transition".to_owned());
    let time = value("--time")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1200.);
    let gather = args.iter().any(|a| a == "--gather");
    let width = value("--width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(if scene_only { WIDTH } else { 1240. });
    let height = value("--height")
        .and_then(|v| v.parse().ok())
        .unwrap_or(if scene_only { HEIGHT } else { 820. });
    let config = value("--config").map(PathBuf::from);
    let debug_view = value("--debug-view").map(|name| {
        serde_json::from_value::<GlimmDebugView>(serde_json::Value::String(name))
            .expect("--debug-view off|mix|displacement|blur|front")
    });
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(
                px(if scene_only { WIDTH } else { 1000. }),
                px(if scene_only { HEIGHT } else { 720. }),
            )),
            is_resizable: !scene_only,
            titlebar: None,
            ..Default::default()
        };
        let window = cx
            .open_window(options, move |window, cx| {
                cx.new(|cx| {
                    let mut view = Playground::new(demo, scene_only, inspect, cx);
                    if let Some(path) = config {
                        let text = std::fs::read_to_string(&path).expect("read config");
                        view.options = merge_config(&view.options, &text).expect("Glimm options");
                        view.element_origin = view.options.at.is_none();
                        view.config_stamp = modified(&path);
                        view.config_path = Some(path);
                    }
                    if let Some(debug_view) = debug_view {
                        view.controller.set_debug_view(debug_view, window);
                    }
                    window.focus(&view.focus, cx);
                    view.start_automatic(window, cx);
                    view
                })
            })
            .expect("open native window");
        cx.activate(true);
        if capture.is_none() {
            // Follow the settings file and the shader, if either was asked for; an ordinary run
            // stops polling after the first look and stays idle.
            cx.spawn(async move |cx| {
                loop {
                    let following =
                        window.update(cx, |view, window, cx| view.follow_files(window, cx));
                    if !following.unwrap_or(false) {
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                }
            })
            .detach();
        }
        if let Some(path) = capture {
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                let _ = window.update(cx, |view, window, cx| {
                    if state == "final" {
                        view.changed = true;
                        view.sheet_button_filled = true;
                        view.overlay = true;
                        view.tasks = 31;
                        view.seconds = 0;
                    } else if state != "initial" {
                        view.target = true;
                        let (work, anticipated) = match view.demo {
                            GlimmDemo::Generate => (2600, false),
                            GlimmDemo::Upgrade => (500, false),
                            GlimmDemo::Publish => (1400, false),
                            GlimmDemo::UsageReset => (0, true),
                            _ => (0, true),
                        };
                        if view.demo == GlimmDemo::Tasks {
                            view.tasks = 31;
                        }
                        let options = view.run_options();
                        let token = if anticipated {
                            view.controller.anticipate_change(options, window, cx)
                        } else {
                            view.controller.anticipate(options, window, cx)
                        };
                        view.pending = Some(token);
                        view.busy = true;
                        view.running = true;
                        let ready = if anticipated {
                            view.options.gather_ms
                        } else {
                            work as f32
                        };
                        let mut snap = 0.;
                        if state != "busy" && !gather && time >= ready {
                            view.controller.seek(ready, window);
                            if ready >= 120.
                                && view.options.page_effect
                                && view.options.effect == GlimmEffect::Scope
                            {
                                snap = 440. * (1. - view.controller.sample(cx).gather);
                            }
                            view.commit(token, window, cx);
                        }
                        view.inspect_burst_age =
                            Some(((time - ready - snap - 90.) / 1000.).max(0.));
                        let fade_end = view.options.fade.map_or(1., |f| f[1]);
                        let exit = 1. - ((1. - fade_end) / 0.92).clamp(0., 1.).powf(1. / 2.4);
                        view.inspect_leave_age = Some(
                            ((time - ready - snap - view.options.sweep_ms * exit) / 1000.).max(0.),
                        );
                        if view.demo == GlimmDemo::UsageReset {
                            view.seconds = 0;
                        }
                        view.controller.seek(time, window);
                        if view.controller.take_midpoint(cx) {
                            view.burst(window, cx);
                        }
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let _ = window.update(cx, |view, _, cx| {
                    eprintln!(
                        "glimm capture: active={}, scope={:?}, at={:?}",
                        view.controller.sample(cx).active,
                        view.controller.bounds(),
                        view.last_at
                    );
                });
                let image = cx.update_window(window.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                    window.render_to_image()
                });
                if let Some(error) = gpui::glimm_shader_error() {
                    eprintln!("glimm shader not applied, built-in shader captured: {error}");
                }
                match image {
                    Ok(Ok(image)) => {
                        image.save(&path).expect("write PNG capture");
                        eprintln!("saved {}", path.display());
                    }
                    Ok(Err(error)) => {
                        eprintln!("capture failed: {error:#}");
                        std::process::exit(2);
                    }
                    Err(error) => {
                        eprintln!("window capture failed: {error:#}");
                        std::process::exit(2);
                    }
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::Playground;
    use gpui::{GlimmDemo, TestAppContext, Window};

    #[gpui::test]
    async fn anchors_match_points_measured_from_original_webgl(cx: &mut TestAppContext) {
        for demo in GlimmDemo::ALL {
            let (view, visual) =
                cx.add_window_view(move |_, cx| Playground::new(demo, true, true, cx));
            visual.run_until_parked();
            view.update(visual, |view, _| {
                // The exporter reaches mask31 by checking Task1 last; real clicks select their own row.
                view.task_origin = 1;
                let options = view.run_options();
                let bounds = view.controller.bounds();
                let actual = options.at.unwrap_or([
                    f32::from(bounds.size.width) * options.origin[0] + options.origin_offset[0],
                    f32::from(bounds.size.height) * options.origin[1] + options.origin_offset[1],
                ]);
                let expected = view.scenes.get(&view.key(true)).effect_origin.unwrap();
                assert!(
                    (actual[0] - expected[0]).abs() < 0.05
                        && (actual[1] - expected[1]).abs() < 0.05,
                    "{demo:?}: native {actual:?}, original {expected:?}"
                );
            });
        }
    }

    #[gpui::test]
    async fn original_reverse_and_dismiss_actions_keep_their_states(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|window: &mut Window, cx| {
            let view = Playground::new(GlimmDemo::Generate, true, true, cx);
            window.focus(&view.focus, cx);
            view
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.changed = true;
                view.sheet_button_filled = true;
                view.trigger(window, cx);
                assert!(!view.changed && !view.busy && view.pending.is_none());
                assert_eq!(
                    view.scene_keys(),
                    ("sheet-initial".into(), "sheet-final".into())
                );
                view.controller.seek(90., window);
                assert!(view.controller.take_midpoint(cx));
                view.burst(window, cx);
                assert_eq!(view.scene_keys().1, "sheet-initial");

                view.select(GlimmDemo::Upgrade, window, cx);
                view.action("Close", window, cx);
                assert!(!view.changed);
                assert_eq!(view.scene_keys().0, "upgrade-closed");
                let closed = view.scenes.get("upgrade-closed");
                assert!(closed.controls.is_empty());
                assert!(closed.nodes.iter().any(|n| n.text == "Free"));

                view.select(GlimmDemo::Publish, window, cx);
                view.changed = true;
                view.trigger(window, cx);
                let token = view.pending.unwrap();
                view.controller.seek(view.options.gather_ms, window);
                view.commit(token, window, cx);
                assert!(!view.changed);
                assert_eq!(view.scene_keys().0, "publish-initial");

                view.select(GlimmDemo::Theme, window, cx);
                view.changed = true;
                assert_eq!(
                    view.run_options().palette,
                    gpui::GlimmPalette::from_colors(&[0x00C8B3, 0x00C0E8, 0x0088FF, 0x6155F5])
                );
                view.palette = Some(gpui::GlimmPaletteName::Spectrum);
                view.options.palette = gpui::GlimmPaletteName::Spectrum.palette();
                assert_eq!(
                    view.run_options().palette,
                    gpui::GlimmPaletteName::Spectrum.palette()
                );
            })
        });
    }

    #[gpui::test]
    async fn keyboard_can_start_and_cancel_without_pointer(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|window: &mut Window, cx| {
            let view = Playground::new(GlimmDemo::Generate, true, true, cx);
            window.focus(&view.focus, cx);
            view
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("tab tab tab enter");
        view.read_with(cx, |view, _| {
            assert!(view.pending.is_some());
            assert_eq!(view.last_at, Some([317., 293.]));
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.pending.is_none()));
    }
}
