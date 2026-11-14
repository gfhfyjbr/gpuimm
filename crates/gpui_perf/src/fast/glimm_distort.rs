//! Glimm's radial distortions on content where bending shows: a grid of lines, text and cards
//! with borders. `--distort NAME` opens this stage instead of the nine glimm.dev examples.
//!
//! Two scopes, as an application would have them on a canvas: `canvas` wraps the whole stage,
//! and `node` wraps a 400 × 300 region around the newest node, nested inside it. The point of
//! action of the node's effects is the parent card's right edge, outside the node's scope.

use gpui::{
    App, AppContext as _, Bounds, Context, ElementId, FocusHandle, GlimmController, GlimmDebugView,
    GlimmOptions, GlimmToken, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, canvas,
    div, fill, glimm_scope, px, rgb, size,
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

/// The stage's default size; `--width` and `--height` change it, `--full` grows nothing but
/// moves the node's effect onto the whole stage.
const STAGE: [f32; 2] = [900., 520.];
/// The parent card, in stage coordinates: x, y, width, height.
const PARENT: [f32; 4] = [60., 170., 240., 170.];
/// The node's own scope, in stage coordinates.
const REGION: [f32; 4] = [420., 110., 400., 300.];
/// The node card inside its scope.
const CARD: [f32; 4] = [60., 60., 280., 180.];
/// Grid spacing, in window coordinates, so the canvas and the node's scope draw the same lines.
const GRID: f32 = 24.;
/// When a held run resolves, in milliseconds after the action.
const RESOLVE_MS: f32 = 1600.;

/// One demonstration of the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Demo {
    /// The node comes out of its parent's edge into a region whose grid it pushes aside.
    Emerge,
    /// A ring of refraction from the node's centre across the whole stage.
    Shock,
    /// The node is drawn back into its parent's edge.
    Collapse,
    /// The canvas breathes around the node while it works, then a shock releases it.
    Hold,
    /// A parent creating a node: the canvas holds at the parent's edge, then ripples while the
    /// node emerges in its own scope. The two-scope setup an application would use.
    Spawn,
}
impl Demo {
    const ALL: [Self; 5] = [
        Self::Emerge,
        Self::Shock,
        Self::Collapse,
        Self::Hold,
        Self::Spawn,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Emerge => "emerge",
            Self::Shock => "shock",
            Self::Collapse => "collapse",
            Self::Hold => "hold",
            Self::Spawn => "spawn",
        }
    }
    pub(super) fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|demo| demo.name() == name)
    }
    /// Whether the card is on the stage before the action.
    fn card_before(self) -> bool {
        !matches!(self, Self::Emerge | Self::Spawn)
    }
    /// Whether the card is on the stage after the action.
    fn card_after(self) -> bool {
        self != Self::Collapse
    }
    /// Whether the run waits for work that resolves at `RESOLVE_MS`.
    fn held(self) -> bool {
        matches!(self, Self::Hold | Self::Spawn)
    }
}

/// The parent's right edge, where the node's effects start, in stage coordinates.
fn parent_edge() -> [f32; 2] {
    [PARENT[0] + PARENT[2], PARENT[1] + PARENT[3] / 2.]
}
/// The node card's centre, in stage coordinates.
fn card_centre() -> [f32; 2] {
    [
        REGION[0] + CARD[0] + CARD[2] / 2.,
        REGION[1] + CARD[1] + CARD[3] / 2.,
    ]
}
fn in_region([x, y]: [f32; 2]) -> [f32; 2] {
    [x - REGION[0], y - REGION[1]]
}

struct Stage {
    focus: FocusHandle,
    demo: Demo,
    /// Run the node's Emerge or Collapse on the whole stage instead of its own region.
    full: bool,
    canvas: GlimmController,
    node: GlimmController,
    card: bool,
    pending: Option<GlimmToken>,
    /// When the node's run started, in milliseconds after the action.
    node_start: Option<f32>,
    /// The instant drawn, in milliseconds after the action, for the label.
    shown_ms: Option<f32>,
    paused: bool,
    epoch: u64,
    config: Option<serde_json::Value>,
    debug: GlimmDebugView,
    shader_stamp: Option<SystemTime>,
    size: [f32; 2],
}

impl Stage {
    fn new(demo: Demo, full: bool, size: [f32; 2], cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            demo,
            full,
            canvas: GlimmController::new(),
            node: GlimmController::new(),
            card: demo.card_before(),
            pending: None,
            node_start: None,
            shown_ms: None,
            paused: false,
            epoch: 0,
            config: None,
            debug: GlimmDebugView::Off,
            shader_stamp: gpui::glimm_shader_stamp(),
            size,
        }
    }
    /// Overlay the `--config` file's keys on a preset.
    fn tuned(&self, options: GlimmOptions) -> GlimmOptions {
        let Some(serde_json::Value::Object(config)) = &self.config else {
            return options;
        };
        let mut merged = serde_json::to_value(&options).expect("options serialize");
        for (key, value) in config {
            merged[key] = value.clone();
        }
        serde_json::from_value(merged).unwrap_or(options)
    }
    /// The node's effect, with its point in the coordinates of the scope that runs it.
    fn node_options(&self) -> GlimmOptions {
        let local = |p| if self.full { p } else { in_region(p) };
        let options = match self.demo {
            Demo::Collapse => GlimmOptions {
                at: Some(local(parent_edge())),
                ..GlimmOptions::collapse()
            },
            _ => GlimmOptions {
                at: Some(local(parent_edge())),
                ..GlimmOptions::emerge()
            },
        };
        self.tuned(options)
    }
    fn canvas_options(&self) -> GlimmOptions {
        let at = match self.demo {
            Demo::Spawn => parent_edge(),
            _ => card_centre(),
        };
        self.tuned(GlimmOptions {
            at: Some(at),
            ..GlimmOptions::shock()
        })
    }
    /// The scope that runs the node's Emerge or Collapse.
    fn node_scope(&self) -> &GlimmController {
        if self.full { &self.canvas } else { &self.node }
    }
    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.canvas.stop(window);
        self.node.stop(window);
        self.card = self.demo.card_before();
        self.pending = None;
        self.node_start = None;
        self.shown_ms = None;
        self.paused = false;
        cx.notify();
    }
    /// Start the demonstration's action. Change state only after the scope captured it.
    fn act(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.demo {
            Demo::Emerge | Demo::Collapse => {
                let options = self.node_options();
                self.node_scope().play(options, window, cx);
                self.node_start = Some(0.);
                self.card = self.demo.card_after();
            }
            Demo::Shock => {
                let options = self.canvas_options();
                self.canvas.play(options, window, cx);
            }
            Demo::Hold | Demo::Spawn => {
                let options = self.canvas_options();
                self.pending = Some(self.canvas.anticipate(options, window, cx));
            }
        }
        cx.notify();
    }
    /// The work behind a held run is done.
    fn resolve(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.pending.take() else {
            return;
        };
        if !self.canvas.resolve(token, window, cx) {
            return;
        }
        if self.demo == Demo::Spawn {
            let options = self.node_options();
            self.node_scope().play(options, window, cx);
            self.node_start = Some(RESOLVE_MS);
            self.card = true;
        }
        cx.notify();
    }
    /// Pause every run at `ms` after the action, resolving on the way when it passes
    /// `RESOLVE_MS`. Instants must come in increasing order.
    fn seek(&mut self, ms: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo.held() && self.pending.is_some() && ms >= RESOLVE_MS {
            self.canvas.seek(RESOLVE_MS, window);
            self.resolve(window, cx);
        }
        self.canvas.seek(ms, window);
        if let Some(start) = self.node_start {
            if !self.full {
                self.node.seek(ms - start, window);
            }
        }
        self.shown_ms = Some(ms);
        self.paused = true;
        cx.notify();
    }
    /// When everything has finished, in milliseconds after the action.
    fn end_ms(&self) -> f32 {
        let canvas = self.canvas_options().validated().sweep_ms;
        let node = self.node_options().validated().sweep_ms;
        match self.demo {
            Demo::Emerge | Demo::Collapse => node,
            Demo::Shock => canvas,
            Demo::Hold => RESOLVE_MS + canvas,
            Demo::Spawn => RESOLVE_MS + canvas.max(node),
        }
    }
    fn replay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset(window, cx);
        // Draw the state before the action once, so the scope has it to capture.
        let epoch = self.epoch;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.epoch == epoch {
                    this.act(window, cx);
                }
            });
            if this
                .read_with(cx, |this, _| this.demo.held() && this.epoch == epoch)
                .unwrap_or(false)
            {
                cx.background_executor()
                    .timer(Duration::from_millis(RESOLVE_MS as u64))
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.epoch == epoch {
                        this.resolve(window, cx);
                    }
                });
            }
        })
        .detach();
    }
    fn set_debug(&mut self, view: GlimmDebugView, window: &mut Window, cx: &mut Context<Self>) {
        self.debug = view;
        self.canvas.set_debug_view(view, window);
        self.node.set_debug_view(view, window);
        cx.notify();
    }
    fn toggle_pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for controller in [&self.canvas, &self.node] {
            if self.paused {
                controller.resume(window, cx);
            } else {
                controller.pause(window, cx);
            }
        }
        self.paused = !self.paused;
        cx.notify();
    }
    /// Step a paused stage by `ms`, every run together.
    fn step(&mut self, ms: f32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.paused {
            return;
        }
        for controller in [&self.canvas, &self.node] {
            if let Some(t) = controller.elapsed_ms(cx) {
                controller.seek((t + ms).max(0.), window);
            }
        }
        cx.notify();
    }

    fn card(&self) -> impl IntoElement {
        let line =
            |width: f32, color: u32| div().h(px(6.)).w(px(width)).rounded(px(3.)).bg(rgb(color));
        div()
            .absolute()
            .left(px(CARD[0]))
            .top(px(CARD[1]))
            .w(px(CARD[2]))
            .h(px(CARD[3]))
            .rounded(px(12.))
            .border_2()
            .border_color(rgb(0x7aa2f7))
            .bg(rgb(0x1b2030))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(10.)).rounded(px(5.)).bg(rgb(0x9ece6a)))
                    .child(
                        div()
                            .text_size(px(15.))
                            .text_color(rgb(0xe6e9f2))
                            .child("agent 2 · reviewer"),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0xa9b1d6))
                    .font_family("Menlo")
                    .child("$ cargo test -p glimm"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0xa9b1d6))
                    .font_family("Menlo")
                    .child("running 41 tests ........"),
            )
            .child(line(220., 0x3b4261))
            .child(line(160., 0x3b4261))
            .child(
                div()
                    .mt_2()
                    .h(px(8.))
                    .w_full()
                    .rounded(px(4.))
                    .bg(rgb(0x2a2f45))
                    .child(div().h_full().w(px(150.)).rounded(px(4.)).bg(rgb(0x7aa2f7))),
            )
    }
    fn parent(&self) -> impl IntoElement {
        div()
            .absolute()
            .left(px(PARENT[0]))
            .top(px(PARENT[1]))
            .w(px(PARENT[2]))
            .h(px(PARENT[3]))
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(0x565f89))
            .bg(rgb(0x1a1b26))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(15.))
                    .text_color(rgb(0xe6e9f2))
                    .child("kou · lead"),
            )
            .children(
                [
                    "spawn reviewer --model sonnet",
                    "wait reviewer",
                    "read reviewer/out.md",
                ]
                .map(|text| {
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0xa9b1d6))
                        .font_family("Menlo")
                        .child(text)
                }),
            )
    }
    /// The node's region: its own grid background in the region demos, so the old content
    /// there is something the wave can visibly push; transparent around the card otherwise.
    fn region(&self) -> impl IntoElement {
        let backed = !self.full && matches!(self.demo, Demo::Emerge | Demo::Collapse);
        let mut region = div()
            .absolute()
            .left(px(REGION[0]))
            .top(px(REGION[1]))
            .w(px(REGION[2]))
            .h(px(REGION[3]));
        if backed {
            region = region.child(grid());
        }
        if self.card {
            region = region.child(self.card());
        }
        glimm_scope("node", &self.node, region)
    }
    fn controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div().flex().flex_wrap().gap_2().p_3();
        for demo in Demo::ALL {
            bar = bar.child(button(
                ElementId::Name(format!("demo-{}", demo.name()).into()),
                demo.name(),
                self.demo == demo,
                cx.listener(move |this, _, window, cx| {
                    this.demo = demo;
                    this.replay(window, cx);
                }),
            ));
        }
        bar = bar
            .child(button(
                "full",
                "whole stage",
                self.full,
                cx.listener(|this, _, window, cx| {
                    this.full = !this.full;
                    this.replay(window, cx);
                }),
            ))
            .child(button(
                "replay",
                "replay",
                false,
                cx.listener(|this, _, window, cx| this.replay(window, cx)),
            ))
            .child(button(
                "pause",
                if self.paused { "resume" } else { "pause" },
                self.paused,
                cx.listener(|this, _, window, cx| this.toggle_pause(window, cx)),
            ));
        for view in GlimmDebugView::ALL {
            bar = bar.child(button(
                ElementId::Name(format!("view-{}", view.name()).into()),
                view.name(),
                self.debug == view,
                cx.listener(move |this, _, window, cx| this.set_debug(view, window, cx)),
            ));
        }
        div()
            .flex()
            .flex_col()
            .child(bar)
            .child(
                div()
                    .px_3()
                    .text_size(px(12.))
                    .text_color(rgb(0x9aa5ce))
                    .child(
                        "Space pauses, arrows step 16 ms (Shift 100 ms), Enter replays. \
                         --dev follows the shader file.",
                    ),
            )
            .children(gpui::glimm_shader_error().map(|error| {
                div()
                    .px_3()
                    .text_size(px(12.))
                    .text_color(rgb(0xf7768e))
                    .child(SharedString::from(format!("shader not applied: {error}")))
            }))
    }
}

/// Grid lines over the element's bounds, at fixed window coordinates.
fn grid() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |bounds: Bounds<Pixels>, _, window, _| {
            window.paint_quad(fill(bounds, rgb(0x14161d)));
            let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let (x1, y1) = (
                x0 + f32::from(bounds.size.width),
                y0 + f32::from(bounds.size.height),
            );
            let mut x = (x0 / GRID).ceil() * GRID;
            while x < x1 {
                let major = (x / GRID).round() as i32 % 4 == 0;
                let line = Bounds::new(
                    gpui::point(px(x), px(y0)),
                    size(px(if major { 1.5 } else { 1. }), px(y1 - y0)),
                );
                window.paint_quad(fill(line, rgb(if major { 0x353b4d } else { 0x22262f })));
                x += GRID;
            }
            let mut y = (y0 / GRID).ceil() * GRID;
            while y < y1 {
                let major = (y / GRID).round() as i32 % 4 == 0;
                let line = Bounds::new(
                    gpui::point(px(x0), px(y)),
                    size(px(x1 - x0), px(if major { 1.5 } else { 1. })),
                );
                window.paint_quad(fill(line, rgb(if major { 0x353b4d } else { 0x22262f })));
                y += GRID;
            }
        },
    )
    .absolute()
    .size_full()
}

fn button(
    id: impl Into<ElementId>,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded(px(6.))
        .border_1()
        .text_size(px(12.))
        .cursor_pointer()
        .bg(rgb(if active { 0x7aa2f7 } else { 0x1f2335 }))
        .text_color(rgb(if active { 0x16161e } else { 0xc0caf5 }))
        .border_color(rgb(if active { 0x7aa2f7 } else { 0x3b4261 }))
        .hover(|s| s.opacity(0.85))
        .on_click(on_click)
        .child(label)
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [w, h] = self.size;
        let edge = parent_edge();
        let card_left = REGION[0] + CARD[0];
        let stage = div()
            .relative()
            .w(px(w))
            .h(px(h))
            .overflow_hidden()
            .child(grid())
            .child(
                div()
                    .absolute()
                    .left(px(60.))
                    .top(px(40.))
                    .w(px(560.))
                    .text_size(px(22.))
                    .text_color(rgb(0xc0caf5))
                    .child("Radial distortion: straight lines should bend"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(60.))
                    .top(px(78.))
                    .w(px(560.))
                    .text_size(px(13.))
                    .text_color(rgb(0x9aa5ce))
                    .child(
                        "The point of action is the parent's right edge; the node's own scope \
                         is the 400 by 300 region on the right.",
                    ),
            )
            // The edge from the parent to the node: a straight line the wave has to bend.
            .child(
                div()
                    .absolute()
                    .left(px(edge[0]))
                    .top(px(edge[1] - 1.))
                    .w(px(card_left - edge[0]))
                    .h(px(2.))
                    .bg(rgb(if self.card { 0x7aa2f7 } else { 0x3b4261 })),
            )
            .child(self.parent())
            .child(
                div()
                    .absolute()
                    .left(px(60.))
                    .top(px(h - 90.))
                    .w(px(w - 120.))
                    .text_size(px(14.))
                    .text_color(rgb(0xa9b1d6))
                    .child(
                        "The quick brown fox jumps over the lazy dog. 0123456789 \
                         The quick brown fox jumps over the lazy dog.",
                    ),
            )
            .child(self.region());
        let label = format!(
            "{}{} · {}",
            self.demo.name(),
            if self.full { " (whole stage)" } else { "" },
            self.shown_ms
                .map_or("live".to_owned(), |ms| format!("{ms:.0} ms")),
        );
        div()
            .id("glimm-distort")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(0x16161e))
            .font_family("Inter")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let step = if event.keystroke.modifiers.shift {
                    100.
                } else {
                    16.
                };
                match event.keystroke.key.as_str() {
                    "space" => this.toggle_pause(window, cx),
                    "enter" => this.replay(window, cx),
                    "left" => this.step(-step, window, cx),
                    "right" => this.step(step, window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .relative()
                    .child(glimm_scope("canvas", &self.canvas, stage))
                    // Drawn after the scopes: the label never bends.
                    .child(
                        div()
                            .absolute()
                            .right(px(12.))
                            .top(px(10.))
                            .px_2()
                            .py_1()
                            .rounded(px(4.))
                            .bg(rgb(0x000000))
                            .text_size(px(12.))
                            .text_color(rgb(0xffffff))
                            .child(label),
                    ),
            )
            .child(self.controls(cx))
    }
}

/// Run the stage. Arguments are the playground's: `--distort NAME [--full] [--time MS |
/// --strip N] [--capture FILE.png] [--debug-view VIEW] [--config FILE.json] [--width W]
/// [--height H]`.
pub(super) fn run(demo: Demo, args: &[String]) {
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|ix| args.get(ix + 1))
            .cloned()
    };
    let full = args.iter().any(|a| a == "--full");
    let capture = value("--capture").map(PathBuf::from);
    let time = value("--time").and_then(|v| v.parse::<f32>().ok());
    let strip = value("--strip").and_then(|v| v.parse::<usize>().ok());
    let width = value("--width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(STAGE[0]);
    let height = value("--height")
        .and_then(|v| v.parse().ok())
        .unwrap_or(STAGE[1]);
    let config = value("--config").map(|path| {
        let text = std::fs::read_to_string(&path).expect("read --config");
        serde_json::from_str::<serde_json::Value>(&text).expect("--config is JSON")
    });
    let debug = value("--debug-view").map(|name| {
        serde_json::from_value::<GlimmDebugView>(serde_json::Value::String(name))
            .expect("--debug-view off|mix|displacement|blur|front")
    });
    gpui_platform::application().run(move |cx: &mut App| {
        let font = std::borrow::Cow::Borrowed(&include_bytes!("glimm_assets/Inter.ttf")[..]);
        cx.text_system()
            .add_fonts(vec![font])
            .expect("bundled Inter font");
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let visible = capture.is_none();
        let window_size = size(px(width), px(height + if visible { 110. } else { 0. }));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                window_size,
                cx,
            ))),
            titlebar: None,
            // A capture never shows a window or takes focus.
            show: visible,
            focus: visible,
            ..Default::default()
        };
        let window = cx
            .open_window(options, move |window, cx| {
                cx.new(|cx| {
                    let mut stage = Stage::new(demo, full, [width, height], cx);
                    stage.config = config;
                    if let Some(view) = debug {
                        stage.set_debug(view, window, cx);
                    }
                    window.focus(&stage.focus, cx);
                    stage
                })
            })
            .expect("open the distortion stage");
        let Some(path) = capture else {
            cx.activate(true);
            let _ = window.update(cx, |stage, window, cx| stage.replay(window, cx));
            if std::env::var_os(gpui::GLIMM_SHADER_DIR_ENV).is_some() {
                // Follow the shader file: the renderer rebuilds when it next draws an effect,
                // a paused one included.
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                        let open = window.update(cx, |stage, window, cx| {
                            let stamp = gpui::glimm_shader_stamp();
                            if stamp != stage.shader_stamp {
                                stage.shader_stamp = stamp;
                                window.refresh();
                                cx.notify();
                            }
                        });
                        if open.is_err() {
                            break;
                        }
                    }
                })
                .detach();
            }
            return;
        };
        cx.spawn(async move |cx| {
            let draw = |cx: &mut gpui::AsyncApp| {
                cx.update_window(window.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                    window.render_to_image()
                })
            };
            // A view shown for the first time measures itself on one frame, lays out on the next.
            for _ in 0..2 {
                let _ = draw(cx);
            }
            let _ = window.update(cx, |stage, window, cx| {
                stage.act(window, cx);
                stage.seek(0., window, cx);
            });
            let end = window
                .update(cx, |stage, _, _| stage.end_ms())
                .unwrap_or(1000.);
            let instants: Vec<f32> = match (strip, time) {
                (Some(count), _) => {
                    let count = count.max(2);
                    (0..count)
                        .map(|i| (end - 1.) * i as f32 / (count - 1) as f32)
                        .collect()
                }
                (None, time) => vec![time.unwrap_or(end * 0.4)],
            };
            let mut frames = Vec::new();
            for ms in instants {
                let _ = window.update(cx, |stage, window, cx| stage.seek(ms, window, cx));
                // Draw twice: the label and the state committed by a resolve land on the first.
                let _ = draw(cx);
                // The second draw is timed: rasterizing the scopes, their mipmaps, the effect
                // pass and the read back of the window, as a rough cost of one frame.
                let started = std::time::Instant::now();
                let image = draw(cx);
                eprintln!(
                    "{ms:.0} ms: drawn and read back in {:.2} ms",
                    started.elapsed().as_secs_f32() * 1000.
                );
                match image {
                    Ok(Ok(image)) => frames.push(image),
                    other => {
                        eprintln!("capture failed: {:?}", other.map(|image| image.map(|_| ())));
                        std::process::exit(2);
                    }
                }
            }
            if let Some(error) = gpui::glimm_shader_error() {
                eprintln!("glimm shader not applied, built-in shader captured: {error}");
            }
            tile(&frames, 4).save(&path).expect("write the capture");
            eprintln!(
                "saved {} ({} frames over {end:.0} ms)",
                path.display(),
                frames.len()
            );
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

/// Lay frames out left to right, `columns` to a row.
fn tile(frames: &[image::RgbaImage], columns: usize) -> image::RgbaImage {
    if frames.len() == 1 {
        return frames[0].clone();
    }
    let (w, h) = frames[0].dimensions();
    let columns = columns.min(frames.len()) as u32;
    let rows = (frames.len() as u32).div_ceil(columns);
    let mut out = image::RgbaImage::new(w * columns, h * rows);
    for (i, frame) in frames.iter().enumerate() {
        let (x, y) = (i as u32 % columns * w, i as u32 / columns * h);
        image::imageops::replace(&mut out, frame, x.into(), y.into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Demo, Stage, in_region, parent_edge};
    use gpui::{TestAppContext, Window};

    #[gpui::test]
    async fn every_demo_starts_its_runs_with_the_point_where_it_belongs(cx: &mut TestAppContext) {
        for demo in Demo::ALL {
            let (view, cx) = cx.add_window_view(move |_: &mut Window, cx| {
                Stage::new(demo, false, super::STAGE, cx)
            });
            cx.run_until_parked();
            cx.update(|window, cx| {
                view.update(cx, |stage, cx| {
                    stage.act(window, cx);
                    stage.seek(super::RESOLVE_MS + 10., window, cx);
                    assert_eq!(stage.card, demo.card_after(), "{demo:?}");
                    let node = stage.node.elapsed_ms(cx).is_some();
                    let canvas = stage.canvas.elapsed_ms(cx).is_some();
                    match demo {
                        Demo::Emerge | Demo::Collapse => assert!(node && !canvas, "{demo:?}"),
                        Demo::Shock | Demo::Hold => assert!(canvas && !node, "{demo:?}"),
                        Demo::Spawn => assert!(canvas && node, "{demo:?}"),
                    }
                    assert!(stage.end_ms() > 0.);
                })
            });
        }
        assert_eq!(in_region(parent_edge()), [-120., 145.]);
    }
}
