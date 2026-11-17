//! GPUI as a view inside a UIKit/SwiftUI or Android Views/Compose application.
//!
//! The native host owns navigation, native controls and the event loop. Keep an
//! `ApplicationHandle` from `Application::run_embedded`, call `tick` from the
//! display callback, and detach before releasing the native surface. All methods
//! run on the same UI thread. Native controls remain ordinary sibling views;
//! their accessibility, Liquid Glass and IME behavior belongs to the OS.

mod callback;
mod dispatcher;
mod platform;
mod surface;
mod window;

pub use gpui_wgpu::CosmicTextSystem;

/// Select a backend when an application's supported driver requires it.
#[derive(Clone, Copy, Debug, Default)]
pub enum GraphicsBackend {
    #[default]
    Auto,
    Metal,
    Vulkan,
    OpenGl,
}
pub use platform::MobilePlatform;
pub use surface::{NativeSurface, Viewport};
