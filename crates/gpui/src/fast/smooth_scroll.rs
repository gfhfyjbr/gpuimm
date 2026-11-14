//! Smooth scrolling: a wheel's line steps glide to where they lead instead of
//! jumping there.
//!
//! A mouse wheel scrolls in steps of lines ([`crate::ScrollDelta::Lines`]),
//! and every scroll container used to move by the whole step at once. Here a
//! step moves the container's target, and its offset glides there over
//! [`SmoothScroll::duration`] on a cubic ease-out: fastest at first, slowing to
//! rest on the target, never past it. A step that comes while a glide is under
//! way moves the target on, and the glide carries on from where it is at the
//! speed it has; a step the other way turns it round at once. See
//! [`ScrollGlide`] for the curve.
//!
//! What is not smoothed:
//! - precise deltas (a trackpad, a mouse that scrolls in pixels), which the
//!   system already smooths: they are applied as they come, as before, and stop
//!   any glide of the container they scroll;
//! - offsets that code sets (`ScrollHandle::set_offset`, `scroll_to_item`, a
//!   list's `scroll_to` or following its end): they apply at once, and a glide
//!   under way gives way to them;
//! - anything while [`SmoothScroll`] is off or the app asks for reduced motion,
//!   and the containers that opt out with `smooth_scroll(false)`.
//!
//! A glide owns its container's offset only while nobody else moves it: each
//! frame it checks the offset is where it left it and stops when it is not,
//! so a scroll that code asks for, or content that shrinks under it, wins at
//! once. It stops too when the container's state is dropped, and it asks for
//! frames only while it moves, so the window stops drawing once it has
//! arrived.
//!
//! Each frame of a glide moves the offset the way a wheel event's listener
//! does and tells the view so in the same way, so for the scroll layers
//! ([`crate::fast::layers`]) a frame of a glide is a frame that only scrolled.
//!
//! Readers of an offset (`ScrollHandle::offset`, a list's
//! `logical_scroll_top` or `is_scrolled_to_end`) see where the content is
//! drawn, not where it is going: a scrollbar, something placed beside a row
//! and a button that appears away from a list's end all describe what is on
//! screen, and a glide bound for a list's end reaches it, so the list follows
//! its end again when it arrives.

use std::{
    cell::RefCell,
    mem,
    rc::{Rc, Weak},
    time::Duration,
};

use scheduler::Instant;
use smallvec::SmallVec;
use sum_tree::Bias;

use crate::fast::layers::invalidate::ScrollContainer;
use crate::{
    App, Div, EntityId, Global, GlobalElementId, InteractiveElement, Interactivity, IsZero, List,
    ListOffset, Pixels, Point, ScrollEvent, StateInner, Stateful, UniformList, Window, point, px,
};

/// How a wheel's line steps scroll the app's scroll areas: whether they glide
/// to where they lead, and how long a glide takes. Set it as a global to
/// change it; without one, [`SmoothScroll::default`] applies: on, for
/// [`SmoothScroll::DEFAULT_DURATION`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothScroll {
    /// Whether line steps glide. On by default; off, each step jumps.
    pub enabled: bool,
    /// How long a glide takes from a standstill. A glide that a further step
    /// catches up with at speed may end sooner, so as not to slow down.
    pub duration: Duration,
}

impl SmoothScroll {
    /// The duration of a glide unless the app sets another: about as long as
    /// a short gesture, enough frames at 60 Hz to read as motion, and over
    /// before the next step of a wheel turned at an easy pace.
    pub const DEFAULT_DURATION: Duration = Duration::from_millis(150);

    /// The setting in effect: the app's, or the default without one. Off
    /// while the app asks for reduced motion ([`App::reduce_motion`]).
    pub fn of(cx: &App) -> Self {
        let setting = cx.try_global::<Self>().copied().unwrap_or_default();
        Self {
            enabled: setting.enabled && !setting.duration.is_zero() && !cx.reduce_motion(),
            ..setting
        }
    }
}

impl Default for SmoothScroll {
    fn default() -> Self {
        Self {
            // Off in this crate's own tests, which send line steps and expect
            // them applied at once, as upstream's do; the tests of smooth
            // scrolling turn it on.
            enabled: !cfg!(test),
            duration: Self::DEFAULT_DURATION,
        }
    }
}

impl Global for SmoothScroll {}

/// A glide of one scroll position towards a target: what a wheel step starts
/// and what further steps move on. GPUI's scroll containers glide with it, and
/// a view that scrolls its own content (a terminal) can use it so its wheel
/// feels the same.
///
/// It eases out on a cubic curve: it starts at its fastest, three times its
/// average speed, and slows to rest on the target without passing it. Sent
/// somewhere else while it moves ([`ScrollGlide::glide_to`]), it starts a new
/// curve from where it is. Towards a target ahead it never slows down: it
/// keeps the speed it has when that is more than a fresh curve would start
/// at, and then arrives sooner than `duration`. Towards a target behind it, it
/// turns at once.
///
/// Positions are in whatever the caller scrolls in, pixels or rows. Times come
/// from the executor's clock ([`crate::BackgroundExecutor::now`]), which tests
/// move by hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollGlide {
    from: f32,
    to: f32,
    /// When the current curve began, while there is one.
    start: Option<Instant>,
    /// How long the current curve takes, in seconds.
    seconds: f32,
}

impl ScrollGlide {
    /// A glide at rest at `position`.
    pub fn new(position: f32) -> Self {
        Self {
            from: position,
            to: position,
            start: None,
            seconds: 0.,
        }
    }

    /// Where the glide is going, or where it rests.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// How far along its curve the glide is at `now`, from 0 to 1.
    fn progress(&self, now: Instant) -> f32 {
        match self.start {
            Some(start) if self.seconds > 0. => {
                (now.saturating_duration_since(start).as_secs_f32() / self.seconds).min(1.)
            }
            _ => 1.,
        }
    }

    /// Where the glide is at `now`: exactly the target once it has arrived.
    pub fn position(&self, now: Instant) -> f32 {
        let progress = self.progress(now);
        if progress >= 1. {
            return self.to;
        }
        let left = 1. - progress;
        self.from + (self.to - self.from) * (1. - left * left * left)
    }

    /// How fast the glide moves at `now`, per second, with the sign of its
    /// direction; zero at rest.
    pub fn velocity(&self, now: Instant) -> f32 {
        let progress = self.progress(now);
        if progress >= 1. {
            return 0.;
        }
        let left = 1. - progress;
        3. * (self.to - self.from) * left * left / self.seconds
    }

    /// Whether the glide is still on its way at `now`.
    pub fn is_moving(&self, now: Instant) -> bool {
        self.progress(now) < 1.
    }

    /// Glides to `target` from wherever it is at `now`, taking `duration` from
    /// a standstill. A zero duration jumps there.
    pub fn glide_to(&mut self, target: f32, now: Instant, duration: Duration) {
        let position = self.position(now);
        let speed = self.velocity(now);
        let distance = target - position;
        let mut seconds = duration.as_secs_f32();
        if distance == 0. || seconds <= 0. {
            *self = Self::new(target);
            return;
        }
        // A fresh curve starts at three times the distance over its duration.
        // A glide already moving towards the target faster than that keeps its
        // speed on a shorter curve, which starts at exactly that speed and
        // still comes to rest on the target.
        if speed * distance > 0. && speed.abs() * seconds > 3. * distance.abs() {
            seconds = 3. * distance.abs() / speed.abs();
        }
        *self = Self {
            from: position,
            to: target,
            start: Some(now),
            seconds,
        };
    }

    /// Stops at `position` at once.
    pub fn jump_to(&mut self, position: f32) {
        *self = Self::new(position);
    }

    /// Moves the glide, where it came from and where it is going, by
    /// `distance`: for content that moved under it.
    pub fn shift(&mut self, distance: f32) {
        self.from += distance;
        self.to += distance;
    }
}

/// Whether an element's wheel steps glide: as the app's [`SmoothScroll`]
/// says, unless the element opted out with `smooth_scroll(false)`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ElementChoice {
    opted_out: bool,
}

impl ElementChoice {
    fn set(&mut self, smooth: bool) {
        self.opted_out = !smooth;
    }
}

impl Div {
    /// Whether a wheel's line steps glide in this scroll container, as the
    /// app's [`SmoothScroll`] says (the default), or jump, for content that
    /// smooths its own scrolling.
    pub fn smooth_scroll(mut self, smooth: bool) -> Self {
        self.interactivity().fast_smooth_scroll.set(smooth);
        self
    }
}

impl<E: InteractiveElement> Stateful<E> {
    /// Whether a wheel's line steps glide in this scroll container, as the
    /// app's [`SmoothScroll`] says (the default), or jump, for content that
    /// smooths its own scrolling.
    pub fn smooth_scroll(mut self, smooth: bool) -> Self {
        self.interactivity().fast_smooth_scroll.set(smooth);
        self
    }
}

impl UniformList {
    /// Whether a wheel's line steps glide in this list, as the app's
    /// [`SmoothScroll`] says (the default), or jump.
    pub fn smooth_scroll(mut self, smooth: bool) -> Self {
        self.interactivity().fast_smooth_scroll.set(smooth);
        self
    }
}

impl List {
    /// Whether a wheel's line steps glide in this list, as the app's
    /// [`SmoothScroll`] says (the default), or jump.
    pub fn smooth_scroll(mut self, smooth: bool) -> Self {
        self.fast_smooth_scroll.set(smooth);
        self
    }
}

/// The glides under way in a window, advanced once a frame while there are
/// any.
#[derive(Default)]
pub(crate) struct Glides {
    containers: Vec<ContainerGlide>,
    lists: Vec<ListGlide>,
    /// Whether the next frame's step is already asked for.
    frame_asked: bool,
}

/// A scroll container's glide: its offset, a glide on each axis, and what its
/// wheel listener tells when the offset moves.
struct ContainerGlide {
    offset: Weak<RefCell<Point<Pixels>>>,
    x: ScrollGlide,
    y: ScrollGlide,
    /// The offset as the glide last left it; any other means something else
    /// moved it.
    left: Point<Pixels>,
    view: EntityId,
    subtrees: SmallVec<[GlobalElementId; 2]>,
    container: ScrollContainer,
}

/// A `list`'s glide. A list keeps its offset as an item and a distance into
/// it, which stays put when rows above it are measured anew, so the glide is
/// applied as the distance it moved since the last frame, as a run of small
/// wheel events would be.
struct ListGlide {
    state: Weak<RefCell<StateInner>>,
    /// How far towards the list's end it has scrolled since it began, in
    /// pixels.
    glide: ScrollGlide,
    /// How much of that the list has been scrolled by.
    applied: f32,
    /// The list's scroll top as the glide last left it.
    left: ListOffset,
    /// Whether the glide is bound for the list's end, which moves on while
    /// content arrives.
    to_end: bool,
    duration: Duration,
    view: EntityId,
}

/// Whether any glide is under way in `window`.
pub(crate) fn gliding(window: &Window) -> bool {
    let glides = &window.fast_layers.glides;
    !glides.containers.is_empty() || !glides.lists.is_empty()
}

/// How long a glide for `event` takes, or `None` when it is not one to glide:
/// a precise delta, an element that opted out, or smooth scrolling off.
fn glide_duration(event: &ScrollEvent, choice: ElementChoice, cx: &App) -> Option<Duration> {
    if event.delta.precise() || choice.opted_out {
        return None;
    }
    let setting = SmoothScroll::of(cx);
    setting.enabled.then_some(setting.duration)
}

/// What a scroll container's wheel listener needs to glide, made where the
/// listener is registered: the container's offset, and whether it opted out.
pub(crate) struct ContainerWheel {
    offset: Weak<RefCell<Point<Pixels>>>,
    choice: ElementChoice,
}

impl ContainerWheel {
    /// For the scroll container `element`, whose offset is `offset`.
    pub(crate) fn painted(element: &Interactivity, offset: &Rc<RefCell<Point<Pixels>>>) -> Self {
        Self {
            offset: Rc::downgrade(offset),
            choice: element.fast_smooth_scroll,
        }
    }

    /// Takes a wheel `event` for the container, whose offset is `offset` and
    /// can go down to `-max`, and which moves it by `delta` on each axis. A
    /// line step that glides starts the container's glide or moves it on, and
    /// leaves `delta` at zero for the listener; anything else is left to the
    /// listener, as before, and stops a glide. Returns whether the event
    /// moves the content, at once or over the glide; when it does not, the
    /// listener does nothing more.
    ///
    /// `subtrees`, `container` and `view` are what the listener invalidates,
    /// notes and notifies when it moves the offset; each frame of the glide
    /// does the same.
    #[allow(clippy::type_complexity)]
    pub(crate) fn wheel(
        wheel: &Self,
        event: &ScrollEvent,
        offset: Point<Pixels>,
        delta: (&mut Pixels, &mut Pixels),
        (max, subtrees, container, view): (
            Point<Pixels>,
            &[GlobalElementId],
            &ScrollContainer,
            EntityId,
        ),
        window: &mut Window,
        cx: &App,
    ) -> bool {
        let (delta_x, delta_y) = delta;
        let step = point(*delta_x, *delta_y);
        let glides = &mut window.fast_layers.glides;
        let known = glides
            .containers
            .iter()
            .position(|glide| glide.offset.ptr_eq(&wheel.offset));
        let Some(duration) = glide_duration(event, wheel.choice, cx) else {
            if let Some(index) = known {
                glides.containers.swap_remove(index);
            }
            return crate::fast::layers::wheel::moves(offset, step, max);
        };

        let (mut x, mut y) = match known.map(|index| &glides.containers[index]) {
            Some(glide) if glide.left == offset => (glide.x, glide.y),
            _ => (
                ScrollGlide::new(offset.x.into()),
                ScrollGlide::new(offset.y.into()),
            ),
        };
        let target = |glide: &ScrollGlide, step: Pixels, max: Pixels| {
            if step.is_zero() {
                glide.target()
            } else {
                (glide.target() + f32::from(step)).clamp(-f32::from(max), 0.)
            }
        };
        let (to_x, to_y) = (target(&x, step.x, max.x), target(&y, step.y, max.y));
        if to_x == x.target() && to_y == y.target() {
            return false;
        }
        let now = cx.background_executor().now();
        if to_x != x.target() {
            x.glide_to(to_x, now, duration);
        }
        if to_y != y.target() {
            y.glide_to(to_y, now, duration);
        }
        let glide = ContainerGlide {
            offset: wheel.offset.clone(),
            x,
            y,
            left: offset,
            view,
            subtrees: subtrees.iter().cloned().collect(),
            container: container.clone(),
        };
        match known {
            Some(index) => glides.containers[index] = glide,
            None => glides.containers.push(glide),
        }
        *delta_x = Pixels::ZERO;
        *delta_y = Pixels::ZERO;
        ask_frame(window);
        true
    }
}

/// What a `list`'s wheel listener needs to glide, made where the listener is
/// registered.
pub(crate) struct ListWheel {
    state: Rc<RefCell<StateInner>>,
    choice: ElementChoice,
    view: EntityId,
}

impl ListWheel {
    /// For `list`, painted by the view current in `window`.
    pub(crate) fn painted(list: &List, window: &Window) -> Self {
        Self {
            state: list.state.0.clone(),
            choice: list.fast_smooth_scroll,
            view: window.current_view(),
        }
    }

    /// Takes a wheel `event` over the list. A line step that glides starts the
    /// list's glide or moves it on, and the listener does nothing more:
    /// returns true. Anything else stops a glide and is the listener's to
    /// handle, as before.
    pub(crate) fn wheel(wheel: &Self, event: &ScrollEvent, window: &mut Window, cx: &App) -> bool {
        let glides = &mut window.fast_layers.glides;
        let known = glides
            .lists
            .iter()
            .position(|glide| glide.state.as_ptr() == Rc::as_ptr(&wheel.state));
        let Some(duration) = glide_duration(event, wheel.choice, cx) else {
            if let Some(index) = known {
                glides.lists.swap_remove(index);
            }
            return false;
        };
        // A list takes a line for twenty pixels, as its own listener does.
        let step = -f32::from(event.delta.pixel_delta(px(20.)).y);
        let Ok(state) = wheel.state.try_borrow() else {
            return false;
        };
        let Some(bounds) = state
            .last_layout_bounds
            .filter(|_| !state.reset && step != 0.)
        else {
            return false;
        };
        let (top, max) = list_position(&state, bounds.size.height);
        let left = state.logical_scroll_top();
        drop(state);

        let (mut glide, applied) = match known.map(|index| &glides.lists[index]) {
            Some(known) if known.left == left => (known.glide, known.applied),
            _ => (ScrollGlide::new(0.), 0.),
        };
        // In the glide's terms, the list's start is at `applied - top` and its
        // end at `applied + max - top`.
        let start = applied - f32::from(top);
        let end = applied + f32::from(max - top);
        let target = (glide.target() + step).clamp(start, end);
        if target == glide.target() {
            return true;
        }
        glide.glide_to(target, cx.background_executor().now(), duration);
        let entry = ListGlide {
            state: Rc::downgrade(&wheel.state),
            glide,
            applied,
            left,
            to_end: target == end,
            duration,
            view: wheel.view,
        };
        match known {
            Some(index) => glides.lists[index] = entry,
            None => glides.lists.push(entry),
        }
        ask_frame(window);
        true
    }
}

/// Where a list's top is, in pixels from its start, and the furthest it can
/// be, counted as its own wheel listener counts them: a list resting at its
/// end is at the furthest.
fn list_position(state: &StateInner, height: Pixels) -> (Pixels, Pixels) {
    let padding = state.last_padding.unwrap_or_default();
    let max = (state.items.summary().height + padding.top + padding.bottom - height).max(px(0.));
    let top = state.scroll_top(&state.logical_scroll_top()).min(max);
    (top, max)
}

/// The scroll top of a list whose top is `top` pixels from its start.
fn offset_at(state: &StateInner, top: Pixels) -> ListOffset {
    let (start, ..) =
        state
            .items
            .find::<crate::ListItemSummary, _>((), &crate::Height(top), Bias::Right);
    ListOffset {
        item_ix: start.count,
        offset_in_item: top - start.height,
    }
}

/// Asks for the next frame's step, once.
fn ask_frame(window: &mut Window) {
    if mem::replace(&mut window.fast_layers.glides.frame_asked, true) {
        return;
    }
    window.on_next_frame(step);
}

/// Moves every glide of `window` to where it is now, before the frame is
/// drawn, and asks for another frame while any is still on its way.
fn step(window: &mut Window, cx: &mut App) {
    window.fast_layers.glides.frame_asked = false;
    let now = cx.background_executor().now();
    let mut containers = mem::take(&mut window.fast_layers.glides.containers);
    containers.retain_mut(|glide| step_container(glide, now, window, cx));
    let mut lists = mem::take(&mut window.fast_layers.glides.lists);
    lists.retain_mut(|glide| step_list(glide, now, window, cx));

    let glides = &mut window.fast_layers.glides;
    glides.containers.append(&mut containers);
    glides.lists.append(&mut lists);
    if gliding(window) {
        ask_frame(window);
    } else {
        crate::fast::layers::hover_freeze::glides_ended(window);
    }
}

/// Moves a container's offset along its glide. Returns whether the glide goes
/// on.
fn step_container(
    glide: &mut ContainerGlide,
    now: Instant,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(cell) = glide.offset.upgrade() else {
        return false;
    };
    let Ok(mut offset) = cell.try_borrow_mut() else {
        return true;
    };
    if *offset != glide.left {
        return false;
    }
    let next = point(px(glide.x.position(now)), px(glide.y.position(now)));
    if next != *offset {
        *offset = next;
        glide.left = next;
        drop(offset);
        crate::fast::retained::invalidate_retained_subtrees(window, &glide.subtrees);
        crate::fast::layers::invalidate::note_scrolled(window, &glide.container);
        cx.notify(glide.view);
    }
    glide.x.is_moving(now) || glide.y.is_moving(now)
}

/// Scrolls a list by what its glide moved since the last frame, the way its
/// wheel listener scrolls it. Returns whether the glide goes on.
fn step_list(glide: &mut ListGlide, now: Instant, window: &mut Window, cx: &mut App) -> bool {
    let Some(cell) = glide.state.upgrade() else {
        return false;
    };
    let Ok(mut state) = cell.try_borrow_mut() else {
        return true;
    };
    if state.logical_scroll_top() != glide.left {
        return false;
    }
    let Some(bounds) = state.last_layout_bounds else {
        return false;
    };
    let height = bounds.size.height;
    let (top, max) = list_position(&state, height);
    if glide.to_end {
        // Rows that arrive while the glide is bound for the end move the end
        // on, and the glide follows it there: the list lands at its end and
        // follows it again, rather than stopping where the end used to be.
        let end = glide.applied + f32::from(max - top);
        if end > glide.glide.target() {
            glide.glide.glide_to(end, now, glide.duration);
        }
    }
    let position = glide.glide.position(now);
    let step = position - glide.applied;
    if step != 0. {
        let scroll_top = offset_at(&state, top);
        state.scroll(
            &scroll_top,
            height,
            point(px(0.), px(-step)),
            glide.view,
            window,
            cx,
        );
        glide.applied = position;
        glide.left = state.logical_scroll_top();
        // An edge stopped it short: there is nowhere further to go.
        let (moved_to, _) = list_position(&state, height);
        if (f32::from(moved_to - top) - step).abs() > 0.5 {
            return false;
        }
    }
    glide.glide.is_moving(now)
}
