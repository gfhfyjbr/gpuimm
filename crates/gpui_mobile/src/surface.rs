use anyhow::{Result, ensure};
use gpui::{DevicePixels, Pixels, Size, px, size};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    HasWindowHandle, UiKitDisplayHandle, UiKitWindowHandle, WindowHandle,
};
use std::{ffi::c_void, ptr::NonNull};

/// View-local dimensions: physical drawable pixels and points/dp scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

impl Viewport {
    pub fn new(width: u32, height: u32, scale: f32) -> Result<Self> {
        ensure!(
            scale.is_finite() && scale > 0.,
            "scale must be finite and positive"
        );
        ensure!(
            width <= i32::MAX as u32 && height <= i32::MAX as u32,
            "drawable dimensions overflow"
        );
        ensure!(
            (width as f32 / scale).is_finite() && (height as f32 / scale).is_finite(),
            "logical dimensions overflow"
        );
        Ok(Self {
            width,
            height,
            scale,
        })
    }
    pub fn logical_size(self) -> Size<Pixels> {
        size(
            px(self.width as f32 / self.scale),
            px(self.height as f32 / self.scale),
        )
    }
    pub(crate) fn drawable_size(self) -> Size<DevicePixels> {
        size(
            DevicePixels(self.width as i32),
            DevicePixels(self.height as i32),
        )
    }
    pub(crate) fn drawable(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Borrowed native target. Construct and use it on the host's UI thread.
/// The host keeps the view/window alive until `MobilePlatform::detach_surface` returns.
#[derive(Clone, Copy, Debug)]
pub struct NativeSurface {
    address: usize,
    ios: bool,
}

impl NativeSurface {
    /// # Safety
    /// `view` must be a live UIView whose backing layer is a CAMetalLayer. It must
    /// remain alive until the renderer is detached or the mobile platform is dropped.
    /// Never use this value or any copy after the native view is released.
    #[cfg(target_os = "ios")]
    pub unsafe fn ui_view(view: NonNull<c_void>) -> Self {
        Self {
            address: view.as_ptr() as usize,
            ios: true,
        }
    }

    /// # Safety
    /// `window` must be an acquired ANativeWindow retained until detachment completes.
    /// Never use this value or any copy after that native reference is released.
    #[cfg(target_os = "android")]
    pub unsafe fn android_window(window: NonNull<c_void>) -> Self {
        Self {
            address: window.as_ptr() as usize,
            ios: false,
        }
    }
}

impl HasWindowHandle for NativeSurface {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let pointer = NonNull::new(self.address as *mut c_void).ok_or(HandleError::Unavailable)?;
        let handle = if self.ios {
            UiKitWindowHandle::new(pointer).into()
        } else {
            AndroidNdkWindowHandle::new(pointer).into()
        };
        // The constructors' lifetime contract is maintained by the native host.
        Ok(unsafe { WindowHandle::borrow_raw(handle) })
    }
}

impl HasDisplayHandle for NativeSurface {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let handle = if self.ios {
            UiKitDisplayHandle::new().into()
        } else {
            AndroidDisplayHandle::new().into()
        };
        Ok(unsafe { DisplayHandle::borrow_raw(handle) })
    }
}

#[cfg(test)]
mod tests {
    use super::Viewport;
    use gpui::{px, size};

    #[test]
    fn dimensions_are_view_local_and_zero_suspends() {
        assert_eq!(
            Viewport::new(1170, 2532, 3.).unwrap().logical_size(),
            size(px(390.), px(844.))
        );
        assert!(!Viewport::new(0, 100, 2.).unwrap().drawable());
        for scale in [0., -1., f32::NAN, f32::INFINITY, f32::MIN_POSITIVE / 16.] {
            assert!(Viewport::new(100, 100, scale).is_err());
        }
        assert!(Viewport::new(u32::MAX, 1, 1.).is_err());
    }
}
