//! Tests of smooth scrolling, a wheel's line steps gliding in GPUI's scroll
//! containers. See [`crate::fast::smooth_scroll`]. Smooth scrolling is off by
//! default in this crate's tests; these turn it on. The app's own tests
//! (`kou-ui`'s `smooth_scroll`) cover the rest of it from outside the crate.

use crate::{
    AppContext as _, InteractiveElement as _, IntoElement, ParentElement as _, PlatformInput,
    Render, ScrollDelta, ScrollEvent, ScrollHandle, SmoothScroll, StatefulInteractiveElement as _,
    Styled as _, TestAppContext, Window, WindowHandle, div, point, px, rgb,
};
use std::time::Duration;

/// One frame at 240 Hz.
const FRAME: Duration = Duration::from_micros(4167);

/// A scroll container of rows, 200 by 100 pixels, at the top left of the window.
struct Page {
    handle: ScrollHandle,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xffffff)).child(
            div()
                .id("scroller")
                .overflow_y_scroll()
                .track_scroll(&self.handle)
                .w(px(200.))
                .h(px(100.))
                .children((0..40u32).map(|index| {
                    div()
                        .w(px(180.))
                        .h(px(20.))
                        .bg(rgb(0x100000 + index * 0x10))
                })),
        )
    }
}

fn open(cx: &mut TestAppContext, layers: bool) -> WindowHandle<Page> {
    cx.update(|cx| {
        cx.set_global(SmoothScroll {
            enabled: true,
            duration: SmoothScroll::DEFAULT_DURATION,
        })
    });
    let window = cx.add_window(|_, _| Page {
        handle: ScrollHandle::new(),
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.set_scroll_layers(layers);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    window
}

fn wheel(cx: &mut TestAppContext, window: WindowHandle<Page>, lines: f32) {
    cx.update_window(window.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position: point(px(20.), px(20.)),
                delta: ScrollDelta::Lines(point(0., lines)),
                ..Default::default()
            }),
            cx,
        );
    })
    .unwrap();
}

fn offset(cx: &mut TestAppContext, window: WindowHandle<Page>) -> f32 {
    window
        .read_with(cx, |page, _| f32::from(page.handle.offset().y))
        .unwrap()
}

/// Runs a frame of `window` as the platform does: its frame callbacks, then
/// the draw. Returns how many callbacks ran.
fn frame(cx: &mut TestAppContext, window: WindowHandle<Page>) -> usize {
    cx.update_window(window.into(), |_, window, cx| {
        let asked = window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
        asked
    })
    .unwrap()
}

/// Whether the scroll container composited its layer in the last frame.
fn composited(window: &mut Window) -> bool {
    let id = window
        .fast_layers
        .scrolls
        .containers()
        .find(|id| id.last() == Some(&"scroller".into()))
        .cloned()
        .expect("the scroll container was painted");
    crate::fast::layers::policy::last_decision(window, &id)
        == Some(crate::fast::layers::policy::Decision::Composite)
}

/// A line step moves nothing when it comes; the frames after it glide the
/// offset to exactly one step on, never back, and then ask for no more.
#[crate::test]
fn a_line_step_glides_to_one_step_and_then_the_frames_stop(cx: &mut TestAppContext) {
    let window = open(cx, false);
    let line = cx
        .update_window(window.into(), |_, window, _| {
            f32::from(window.line_height())
        })
        .unwrap();
    wheel(cx, window, -2.);
    assert_eq!(offset(cx, window), 0.);
    let mut offsets = vec![0.];
    loop {
        cx.executor().advance_clock(FRAME);
        if frame(cx, window) == 0 {
            break;
        }
        offsets.push(offset(cx, window));
    }
    assert_eq!(*offsets.last().unwrap(), -2. * line, "{offsets:?}");
    assert!(
        offsets.windows(2).all(|pair| pair[1] <= pair[0]),
        "{offsets:?}"
    );
    assert!(offsets.len() > 20, "{offsets:?}");
    let gliding = cx
        .update_window(window.into(), |_, window, _| {
            crate::fast::smooth_scroll::gliding(window)
        })
        .unwrap();
    assert!(!gliding);
}

/// Each frame of a glide moves the offset as a wheel event's listener does,
/// so the scroll layer composites it like a frame of trackpad scrolling, and
/// draws what a window without layers draws.
#[crate::test]
fn the_frames_of_a_glide_are_composited_and_draw_what_they_would_without(cx: &mut TestAppContext) {
    if !crate::fast::layers::COMPILED {
        return;
    }
    let layered = open(cx, true);
    let plain = open(cx, false);
    let mut frames = 0;
    let mut composited_frames = 0;
    for lines in [-3., -2., 1.] {
        wheel(cx, layered, lines);
        wheel(cx, plain, lines);
        loop {
            cx.executor().advance_clock(FRAME);
            let asked = [layered, plain].map(|window| frame(cx, window));
            assert_eq!(asked[0], asked[1]);
            if asked[0] == 0 {
                break;
            }
            frames += 1;
            let drawn = [layered, plain].map(|window| {
                cx.update_window(window.into(), |_, window, _| window.painted_primitives())
                    .unwrap()
            });
            assert_eq!(drawn[0], drawn[1], "frame {frames}");
            assert_eq!(offset(cx, layered), offset(cx, plain));
            if cx
                .update_window(layered.into(), |_, window, _| composited(window))
                .unwrap()
            {
                composited_frames += 1;
            }
        }
    }
    assert!(
        composited_frames * 4 >= frames * 3,
        "composited {composited_frames} of {frames} frames"
    );
}
