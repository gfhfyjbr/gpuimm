use crate::{NativeSurface, Viewport, callback::Callback};
use gpui::{
    AnyWindowHandle, Bounds, Capslock, DispatchEventResult, GpuSpecs, Modifiers, Pixels,
    PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point,
    PromptButton, PromptLevel, RequestFrameOptions, Scene, Size, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowInsets, WindowVisibility,
};
use gpui_wgpu::{GpuContext, WgpuContext, WgpuRenderer, WgpuSurfaceConfig};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

pub(crate) struct WindowState {
    pub renderer: RefCell<WgpuRenderer>,
    pub context: GpuContext,
    pub surface: Cell<Option<NativeSurface>>,
    pub viewport: Cell<Viewport>,
    pub appearance: Cell<WindowAppearance>,
    pub active: Cell<bool>,
    pub visible: Cell<bool>,
    pub force_frame: Cell<bool>,
    pub handle: Cell<Option<AnyWindowHandle>>,
    pub insets: RefCell<WindowInsets>,
    pub mouse: Cell<Point<Pixels>>,
    pub display: Rc<crate::platform::Display>,
    pub frame: Callback<Box<dyn FnMut(RequestFrameOptions)>>,
    pub input: Callback<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    pub resize: Callback<Box<dyn FnMut(Size<Pixels>, f32)>>,
    pub activated: Callback<Box<dyn FnMut(bool)>>,
    pub visibility: Callback<Box<dyn FnMut(WindowVisibility)>>,
    pub appearance_changed: Callback<Box<dyn FnMut()>>,
    pub insets_changed: Callback<Box<dyn FnMut(WindowInsets)>>,
    pub input_handler: Callback<PlatformInputHandler>,
    close: Callback<Box<dyn FnOnce()>>,
}

impl WindowState {
    pub fn new(
        surface: NativeSurface,
        viewport: Viewport,
        backend: crate::GraphicsBackend,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(viewport.drawable(), "initial view must have a nonzero size");
        let context = Rc::new(RefCell::new(None));
        let backends = match backend {
            crate::GraphicsBackend::Auto => None,
            crate::GraphicsBackend::Metal => Some(wgpu::Backends::METAL),
            crate::GraphicsBackend::Vulkan => Some(wgpu::Backends::VULKAN),
            crate::GraphicsBackend::OpenGl => Some(wgpu::Backends::GL),
        };
        if let Some(backends) = backends {
            let mut flags = wgpu::InstanceFlags::default();
            if cfg!(target_os = "android") {
                flags |= wgpu::InstanceFlags::DISCARD_HAL_LABELS;
            }
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                flags,
                display: Some(Box::new(surface)),
                backend_options: wgpu::BackendOptions::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            });
            let target = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&surface)? };
            let probe = unsafe { instance.create_surface_unsafe(target)? };
            *context.borrow_mut() = Some(WgpuContext::new(
                instance,
                &probe,
                &Self::config(viewport),
                None,
            )?);
            // Selection configures its probe; drop that swapchain before the
            // renderer creates its own surface on the same native window.
            drop(probe);
        }
        let renderer = WgpuRenderer::new(context.clone(), &surface, Self::config(viewport), None)?;
        Ok(Self {
            renderer: RefCell::new(renderer),
            context,
            surface: Cell::new(Some(surface)),
            viewport: Cell::new(viewport),
            appearance: Cell::new(WindowAppearance::Light),
            active: Cell::new(false),
            visible: Cell::new(false),
            force_frame: Cell::new(true),
            handle: Cell::new(None),
            insets: RefCell::default(),
            mouse: Cell::default(),
            display: Rc::new(crate::platform::Display(Cell::new(viewport))),
            frame: Callback::default(),
            input: Callback::default(),
            resize: Callback::default(),
            activated: Callback::default(),
            visibility: Callback::default(),
            appearance_changed: Callback::default(),
            insets_changed: Callback::default(),
            input_handler: Callback::default(),
            close: Callback::default(),
        })
    }
    pub fn config(viewport: Viewport) -> WgpuSurfaceConfig {
        WgpuSurfaceConfig {
            size: viewport.drawable_size(),
            transparent: false,
            preferred_present_mode: None,
        }
    }
    pub fn visibility(&self) -> WindowVisibility {
        if self.visible.get() && self.surface.get().is_some() && self.viewport.get().drawable() {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        }
    }
}

pub(crate) struct MobileWindow(pub Rc<WindowState>);

impl HasWindowHandle for MobileWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let surface = self.0.surface.get().ok_or(HandleError::Unavailable)?;
        Ok(unsafe { WindowHandle::borrow_raw(surface.window_handle()?.as_raw()) })
    }
}
impl HasDisplayHandle for MobileWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let surface = self.0.surface.get().ok_or(HandleError::Unavailable)?;
        Ok(unsafe { DisplayHandle::borrow_raw(surface.display_handle()?.as_raw()) })
    }
}

impl Drop for MobileWindow {
    fn drop(&mut self) {
        self.0.handle.set(None);
        self.0.renderer.borrow_mut().unconfigure_surface();
        self.0.surface.set(None);
        self.0.frame.take();
        self.0.input.take();
        self.0.input_handler.take();
        if let Some(callback) = self.0.close.take() {
            callback();
        }
    }
}

impl PlatformWindow for MobileWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: Point::default(),
            size: self.content_size(),
        }
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.0.viewport.get().logical_size()
    }
    fn resize(&mut self, _: Size<Pixels>) {}
    fn scale_factor(&self) -> f32 {
        self.0.viewport.get().scale
    }
    fn appearance(&self) -> WindowAppearance {
        self.0.appearance.get()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.0.display.clone())
    }
    fn mouse_position(&self) -> Point<Pixels> {
        self.0.mouse.get()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }
    fn capslock(&self) -> Capslock {
        Capslock { on: false }
    }
    fn set_input_handler(&mut self, handler: PlatformInputHandler) {
        self.0.input_handler.set(handler);
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.input_handler.take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {}
    fn is_active(&self) -> bool {
        self.0.active.get()
    }
    fn visibility(&self) -> WindowVisibility {
        self.0.visibility()
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, _: &str) {}
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        let state = Rc::downgrade(&self.0);
        Some(Rc::new(move || {
            if let Some(state) = state.upgrade() {
                state.force_frame.set(true);
            }
        }))
    }
    fn schedule_frame(&self) {
        self.0.force_frame.set(true);
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.frame.set(callback);
    }
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.input.set(callback);
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.activated.set(callback);
    }
    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.0.visibility.set(callback);
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.resize.set(callback);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_display_changed(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.close.set(callback);
    }
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.appearance_changed.set(callback);
    }
    fn draw(&self, scene: &Scene) {
        if self.0.visibility().is_visible() {
            self.0.renderer.borrow_mut().draw(scene);
        }
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.renderer.borrow().sprite_atlas().clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.renderer.borrow().gpu_specs()
    }
    fn update_ime_position(&self, _: Bounds<Pixels>) {}
    fn insets(&self) -> WindowInsets {
        self.0.insets.borrow().clone()
    }
    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.0.insets_changed.set(callback);
    }
}
