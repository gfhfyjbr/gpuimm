use anyhow::Context as _;
use gpui::Scene;

#[cfg(feature = "x11")]
pub(crate) fn x11(
    window: &crate::linux::X11Window,
    scene: &Scene,
) -> anyhow::Result<image::RgbaImage> {
    window
        .0
        .state
        .borrow_mut()
        .renderer
        .as_mut()
        .context("X11 window has no renderer")?
        .render_to_image(scene)
}

#[cfg(feature = "wayland")]
pub(crate) fn wayland(
    window: &crate::linux::wayland::window::WaylandWindow,
    scene: &Scene,
) -> anyhow::Result<image::RgbaImage> {
    window
        .borrow_mut()
        .renderer
        .as_mut()
        .context("Wayland window has no renderer")?
        .render_to_image(scene)
}
