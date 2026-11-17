use crate::{
    NativeSurface, Viewport,
    callback::Callback,
    dispatcher::Dispatcher,
    window::{MobileWindow, WindowState},
};
use anyhow::{Result, anyhow, ensure};
use futures::channel::oneshot;
use gpui::{
    Action, ActivityGuard, AnyWindowHandle, AppLifecyclePhase, BackgroundExecutor, Bounds,
    ClipboardItem, CursorStyle, DispatchEventResult, DisplayId, DummyKeyboardMapper,
    ForegroundExecutor, Keymap, Menu, MenuItem, OperatingSystem, PathPromptOptions, Pixels,
    Platform, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTarget, PlatformTextSystem, PlatformWindow, Point,
    RequestFrameOptions, Task, ThermalState, TouchEvent, TouchId, TouchPhase, WindowAppearance,
    WindowInsets, WindowKind, WindowParams,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

/// One host-owned native view and one GPUI window. Create one instance per view.
/// This type is deliberately `!Send`/`!Sync`; native callbacks stay on the UI thread.
pub struct MobilePlatform {
    window: Rc<WindowState>,
    dispatcher: Arc<Dispatcher>,
    text: Arc<dyn PlatformTextSystem>,
    opened: Cell<bool>,
    touches: RefCell<BTreeMap<TouchId, TouchEvent>>,
    lifecycle: Callback<Box<dyn FnMut(AppLifecyclePhase)>>,
    memory_warning: Callback<Box<dyn FnMut()>>,
}

impl Drop for MobilePlatform {
    fn drop(&mut self) {
        self.dispatcher.shutdown();
    }
}

impl MobilePlatform {
    /// # Safety
    /// Keep `surface` alive until detachment completes or both the ApplicationHandle
    /// and platform are dropped. Call on the native UI thread, after layout.
    pub unsafe fn new(
        surface: NativeSurface,
        viewport: Viewport,
        text: Arc<dyn PlatformTextSystem>,
    ) -> Result<Rc<Self>> {
        unsafe { Self::new_with_backend(surface, viewport, text, crate::GraphicsBackend::Auto) }
    }

    /// Construct a view with an explicit graphics backend.
    ///
    /// # Safety
    /// The native lifetime and UI-thread requirements are identical to `new`.
    pub unsafe fn new_with_backend(
        surface: NativeSurface,
        viewport: Viewport,
        text: Arc<dyn PlatformTextSystem>,
        backend: crate::GraphicsBackend,
    ) -> Result<Rc<Self>> {
        let viewport = Viewport::new(viewport.width, viewport.height, viewport.scale)?;
        Ok(Rc::new(Self {
            window: Rc::new(WindowState::new(surface, viewport, backend)?),
            dispatcher: Dispatcher::new(),
            text,
            opened: Cell::new(false),
            touches: RefCell::default(),
            lifecycle: Callback::default(),
            memory_warning: Callback::default(),
        }))
    }

    /// Pump queued foreground tasks, then request a frame if the view is visible.
    /// Drive this from CADisplayLink or Choreographer; do not call inside `App::update`.
    pub fn tick(&self) -> Result<()> {
        self.dispatcher.drain();
        if !self.window.visibility().is_visible() {
            return Ok(());
        }
        ensure!(
            !self.window.renderer.borrow().device_lost(),
            "GPU device lost: recreate the mobile application"
        );
        let force = self.window.force_frame.replace(false);
        self.window.frame.call(|callback| {
            callback(RequestFrameOptions {
                force_render: force,
                require_presentation: force,
                ..Default::default()
            })
        });
        Ok(())
    }

    pub fn resize(&self, viewport: Viewport) -> Result<()> {
        let viewport = Viewport::new(viewport.width, viewport.height, viewport.scale)?;
        let max = self.window.renderer.borrow().max_texture_size();
        ensure!(
            viewport.width <= max && viewport.height <= max,
            "drawable exceeds GPU limit {max}"
        );
        if self.window.viewport.get() == viewport {
            return Ok(());
        }
        let before = self.window.visibility();
        self.window.viewport.set(viewport);
        self.window.display.0.set(viewport);
        if viewport.drawable() && self.window.surface.get().is_some() {
            self.window
                .renderer
                .borrow_mut()
                .update_drawable_size(viewport.drawable_size());
        }
        self.window.force_frame.set(true);
        self.window
            .resize
            .call(|callback| callback(viewport.logical_size(), viewport.scale));
        let after = self.window.visibility();
        if before != after {
            if !after.is_visible() {
                self.cancel_touches();
            }
            self.window.visibility.call(|callback| callback(after));
        }
        Ok(())
    }

    /// Release the swapchain before the OS releases its UIView/ANativeWindow.
    /// The device and atlas survive; `attach_surface` resumes the same application.
    pub fn detach_surface(&self) {
        self.cancel_touches();
        let before = self.window.visibility();
        self.window.renderer.borrow_mut().unconfigure_surface();
        self.window.surface.set(None);
        if before.is_visible() {
            self.window
                .visibility
                .call(|callback| callback(self.window.visibility()));
        }
    }

    /// Permanently stop this application before dropping its ApplicationHandle.
    /// Cancels queued work that could otherwise retain the application/executor.
    /// For temporary surface loss, use `detach_surface` instead.
    pub fn shutdown(&self) {
        self.detach_surface();
        self.dispatcher.shutdown();
    }

    /// # Safety
    /// Same native lifetime/thread contract as `new`. Detach the previous surface first.
    pub unsafe fn attach_surface(&self, surface: NativeSurface, viewport: Viewport) -> Result<()> {
        ensure!(
            self.window.surface.get().is_none(),
            "detach the previous surface before replacing it"
        );
        ensure!(
            viewport.drawable(),
            "replacement surface must have a nonzero size"
        );
        self.resize(viewport)?;
        let instance = self
            .window
            .context
            .borrow()
            .as_ref()
            .ok_or_else(|| anyhow!("GPU context unavailable"))?
            .instance
            .clone();
        self.window.renderer.borrow_mut().replace_surface(
            &surface,
            WindowState::config(viewport),
            &instance,
        )?;
        self.window.surface.set(Some(surface));
        self.window.force_frame.set(true);
        self.window
            .visibility
            .call(|callback| callback(self.window.visibility()));
        Ok(())
    }

    pub fn set_lifecycle(&self, phase: AppLifecyclePhase) {
        let (active, visible) = match phase {
            AppLifecyclePhase::Active => (true, true),
            AppLifecyclePhase::Foreground | AppLifecyclePhase::Inactive => (false, true),
            AppLifecyclePhase::Background | AppLifecyclePhase::Disconnected => (false, false),
        };
        if !active {
            self.cancel_touches();
        }
        let before = self.window.visibility();
        self.window.visible.set(visible);
        if self.window.active.replace(active) != active {
            self.window.activated.call(|callback| callback(active));
        }
        if before != self.window.visibility() {
            self.window
                .visibility
                .call(|callback| callback(self.window.visibility()));
            self.window.force_frame.set(true);
        }
        self.lifecycle.call(|callback| callback(phase));
    }

    pub fn set_appearance(&self, appearance: WindowAppearance) {
        if self.window.appearance.replace(appearance) != appearance {
            self.window.appearance_changed.call(|callback| callback());
            self.window.force_frame.set(true);
        }
    }

    /// Insets are local points/dp. A view already inside the host's safe area uses zero.
    pub fn set_insets(&self, insets: WindowInsets) {
        if *self.window.insets.borrow() != insets {
            self.window.insets.replace(insets.clone());
            self.window.insets_changed.call(|callback| callback(insets));
        }
    }

    pub fn memory_warning(&self) {
        self.memory_warning.call(|callback| callback());
    }

    /// Non-touch input for hardware keyboards/pointers supplied by the host.
    pub fn input(&self, input: PlatformInput) -> DispatchEventResult {
        if !self.window.active.get() || !self.window.visibility().is_visible() {
            return DispatchEventResult::default();
        }
        self.window
            .input
            .call(|callback| callback(input))
            .unwrap_or_default()
    }

    /// Touch positions must be relative to the GPU view, in points/dp. Native sibling
    /// controls are hit-tested by the host and must not forward their touches here.
    pub fn touch(&self, event: TouchEvent) -> DispatchEventResult {
        if !self.window.active.get() || !self.window.visibility().is_visible() {
            return DispatchEventResult::default();
        }
        if !f32::from(event.position.x).is_finite() || !f32::from(event.position.y).is_finite() {
            return DispatchEventResult::default();
        }
        {
            let mut touches = self.touches.borrow_mut();
            match event.phase {
                TouchPhase::Started => {
                    if touches.contains_key(&event.id) {
                        return DispatchEventResult::default();
                    }
                    touches.insert(event.id, event.clone());
                }
                TouchPhase::Moved => {
                    let Some(previous) = touches.get_mut(&event.id) else {
                        return DispatchEventResult::default();
                    };
                    *previous = event.clone();
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if touches.remove(&event.id).is_none() {
                        return DispatchEventResult::default();
                    }
                }
            }
        }
        self.window.mouse.set(event.position);
        self.input(PlatformInput::Touch(event))
    }

    fn cancel_touches(&self) {
        let touches = std::mem::take(&mut *self.touches.borrow_mut());
        for (_, mut event) in touches {
            event.phase = TouchPhase::Cancelled;
            self.window
                .input
                .call(|callback| callback(PlatformInput::Touch(event)));
        }
    }

    /// For a host implementing UITextInput/InputConnection. Native TextField/EditText
    /// controls need no GPUI IME bridge and retain their own accessibility semantics.
    pub fn with_input_handler<R>(
        &self,
        f: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        self.window.input_handler.call(f)
    }
}

#[derive(Debug)]
pub(crate) struct Display(pub Cell<Viewport>);
impl PlatformDisplay for Display {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::nil())
    }
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: Point::default(),
            size: self.0.get().logical_size(),
        }
    }
    fn refresh_interval(&self) -> Option<Duration> {
        None
    }
}
struct Keyboard;
impl PlatformKeyboardLayout for Keyboard {
    fn id(&self) -> &str {
        "native-host"
    }
    fn name(&self) -> &str {
        "Native host"
    }
}
fn unsupported<T>() -> Result<T> {
    Err(anyhow!("this operation belongs to the native mobile host"))
}
fn unsupported_receiver<T>() -> oneshot::Receiver<Result<T>> {
    let (sender, receiver) = oneshot::channel();
    let _ = sender.send(unsupported());
    receiver
}

impl Platform for MobilePlatform {
    fn platform_info(&self) -> gpui::PlatformInfo {
        gpui::PlatformInfo {
            os: if cfg!(target_os = "ios") {
                OperatingSystem::Ios
            } else {
                OperatingSystem::Android
            },
            target: PlatformTarget::Native,
        }
    }
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, launched: Box<dyn FnOnce()>) {
        launched();
    }
    fn quit(&self) {
        self.set_lifecycle(AppLifecyclePhase::Disconnected);
    }
    fn restart(&self, _: Option<PathBuf>, _: Vec<OsString>) {}
    fn activate(&self, _: bool) {}
    fn hide(&self) {}
    fn hide_other_apps(&self) {}
    fn unhide_other_apps(&self) {}
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.window.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.window.display.clone())
    }
    fn on_displays_changed(&self, _: Box<dyn FnMut()>) {}
    fn active_window(&self) -> Option<AnyWindowHandle> {
        if self.window.active.get() {
            self.window.handle.get()
        } else {
            None
        }
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        ensure!(
            matches!(options.kind, WindowKind::Normal),
            "mobile embedded views support normal windows only"
        );
        ensure!(
            !self.opened.replace(true),
            "one GPUI window per native view; use another MobilePlatform for another view"
        );
        self.window.handle.set(Some(handle));
        Ok(Box::new(MobileWindow(self.window.clone())))
    }
    fn window_appearance(&self) -> WindowAppearance {
        self.window.appearance.get()
    }
    fn open_url(&self, _: &str) {}
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        unsupported_receiver()
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        unsupported_receiver()
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn on_quit(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_reopen(&self, _: Box<dyn FnMut()>) {}
    fn on_system_sleep(&self, _: Box<dyn FnMut()>) {}
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}
    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.lifecycle.set(callback);
    }
    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.memory_warning.set(callback);
    }
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) {}
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) {}
    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }
    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}
    fn prevent_idle_sleep(&self, _: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(unsupported())
    }
    fn app_path(&self) -> Result<PathBuf> {
        unsupported()
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        unsupported()
    }
    fn set_cursor_style(&self, _: CursorStyle) {}
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        true
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }
    fn write_to_clipboard(&self, _: ClipboardItem) {}
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {}
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _: ClipboardItem) {}
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(unsupported())
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(unsupported())
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(Keyboard)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}
