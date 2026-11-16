//! The window zoom: Chromium's page zoom done the way Chromium does it,
//! as a device pixel ratio.
//!
//! Obsidian's `window:zoom-in`, `window:zoom-out` and `window:reset-zoom`
//! move the vault window's zoom level by 0.5 between −2.5 and +3, the
//! factor being 1.2^level, and the window then reports
//! `devicePixelRatio` = display scale × factor (see
//! [[Specification#Window zoom]]). Scoria paints in logical pixels, some
//! two thousand literals of them, and gpui keeps a window's scale factor
//! to itself, so the zoom is applied one layer down: [`ZoomPlatform`]
//! wraps the platform gpui is started on, and every window it opens is a
//! [`ZoomWindow`] that reports the display's scale times the factor and a
//! content size divided by it, and divides each pointer position by it on
//! the way in. gpui then lays out a smaller logical window and rasterizes
//! everything, text included, at the larger scale: the whole window zooms,
//! crisp, with no element knowing. A zoom change is
//! [`Window::bounds_changed`], which re-reads both.
//!
//! What the wrapper cannot reach is the IME's caret rectangle, which the
//! platform asks the input handler for in logical pixels behind gpui's
//! private `PlatformInputHandler`: at a zoom other than 0 a composition
//! window opens that factor off the caret.
//!
//! The same wrapper is where a window's always-on-top lives
//! ([`set_always_on_top`]): gpui fixes a window's level when it opens it,
//! and the native handle the wrapper holds is what each desktop's own call
//! takes.

use futures::channel::oneshot;
use gpui::{
    AnyWindowHandle, App, BackgroundExecutor, Bounds, Capslock, ClipboardItem, CursorStyle,
    Decorations, DispatchEventResult, FileDropEvent, ForegroundExecutor, GpuSpecs, Keymap, Menu,
    MenuItem, Modifiers, OwnedMenu, PathPromptOptions, Pixels, Platform, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Point, PromptButton, PromptLevel,
    RequestFrameOptions, ResizeEdge, Scene, ScreenCaptureSource, ScrollDelta, Size,
    SystemWindowTab, Task, ThermalState, Window, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowButtonLayout, WindowControlArea, WindowControls, WindowDecorations,
    WindowId, WindowParams,
};
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use smallvec::SmallVec;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// One press of Zoom in or Zoom out.
pub const STEP: f32 = 0.5;
/// The highest level Zoom in reaches (factor 1.728).
pub const MAX_LEVEL: f32 = 3.0;
/// The lowest level Zoom out reaches (factor 0.6339).
pub const MIN_LEVEL: f32 = -2.5;

/// The factor a zoom level stands for: Chromium's 1.2^level.
pub fn factor(level: f32) -> f32 {
    1.2f32.powf(level)
}

/// The level one press of Zoom in (`+1`) or Zoom out (`-1`) leaves, from
/// `level`: a half step, held inside the limits, the way Obsidian's
/// twentieth press past a limit changed nothing.
pub fn stepped(level: f32, direction: i32) -> f32 {
    let next = level + STEP * direction.signum() as f32;
    // Levels are halves; snap, so a value read from a file as 0.49999
    // does not drift.
    ((next * 2.0).round() / 2.0).clamp(MIN_LEVEL, MAX_LEVEL)
}

/// The zoom and the on-top flag of one window, shared between the
/// [`ZoomWindow`] that applies them and [`REGISTRY`], which is how the
/// workspace reaches them through a gpui `Window`.
/// A desktop's own call that keeps a window on top (true) or stops;
/// true when the desktop took the request.
type OnTop = Box<dyn Fn(bool) -> bool>;

#[derive(Default)]
struct WindowState {
    /// The zoom level, 0 for none.
    level: Cell<f32>,
    /// Whether the window was asked to stay above the others.
    on_top: Cell<bool>,
    /// Asks the native window to stay above (true) or not; set by the
    /// wrapper when it opens the window, `None` where the desktop has no
    /// way to say it.
    apply_on_top: Option<OnTop>,
}

thread_local! {
    static REGISTRY: RefCell<HashMap<WindowId, Rc<WindowState>>> = RefCell::new(HashMap::new());
}

/// gpui's id for `window` (named in full: raw-window-handle's trait of the
/// same method name is in scope here).
fn id_of(window: &Window) -> WindowId {
    Window::window_handle(window).window_id()
}

fn state_of(id: WindowId) -> Option<Rc<WindowState>> {
    REGISTRY.with(|r| r.borrow().get(&id).cloned())
}

/// The zoom level of `window`, 0 for none (and for a window not opened
/// through [`ZoomPlatform`], as in tests).
pub fn level(window: &Window) -> f32 {
    state_of(id_of(window)).map_or(0.0, |s| s.level.get())
}

/// Sets `window`'s zoom level, held inside the limits, and relays the
/// window out at it. Returns the level set.
pub fn set_level(window: &mut Window, cx: &mut App, level: f32) -> f32 {
    let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
    let Some(state) = state_of(id_of(window)) else {
        return 0.0;
    };
    if state.level.get() != level {
        state.level.set(level);
        window.bounds_changed(cx);
    }
    level
}

/// `window`'s size on the desktop (`window_bounds`, in the desktop's
/// logical pixels) in the zoomed window's own pixels, the ones its pointer
/// positions and its layout are in.
pub fn window_size(window: &Window) -> Size<Pixels> {
    let f = factor(level(window));
    let s = window.window_bounds().get_bounds().size;
    Size::new(s.width / f, s.height / f)
}

/// Where the vault window's zoom is kept between launches:
/// `window/<vault id>.json` under Scoria's configuration folder. Obsidian
/// keeps it in its data directory's `<vault id>.json`, the window file, as
/// `zoom` beside the window's bounds; Scoria's file has the same name and
/// key in Scoria's folder, never in the vault and never in Obsidian's. The
/// text size the font-size commands set is kept in the same file, under
/// `base-font-size` (see [`crate::theme::fonts::STORE_KEY`]).
pub(crate) fn window_file(root: &Path) -> Option<PathBuf> {
    window_file_in(&scoria_vault::registry::config_dir()?, root)
}

fn window_file_in(config: &Path, root: &Path) -> Option<PathBuf> {
    Some(
        config
            .join("window")
            .join(format!("{}.json", crate::fold::vault_id(root))),
    )
}

/// The zoom level recorded for the vault at `root`, 0 when none is.
pub fn load_level(root: &Path) -> f32 {
    window_file(root).map_or(0.0, |p| read_level(&p))
}

fn read_level(path: &Path) -> f32 {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("zoom").and_then(serde_json::Value::as_f64))
        .map_or(0.0, |z| (z as f32).clamp(MIN_LEVEL, MAX_LEVEL))
}

/// Records `level` for the vault at `root`, keeping any other key the file
/// holds. A failure to write costs only the next launch's zoom.
pub fn save_level(root: &Path, level: f32) {
    if let Some(path) = window_file(root) {
        write_level(&path, level);
    }
}

fn write_level(path: &Path, level: f32) {
    let mut json = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    // Whole levels as integers, the way Obsidian's file has `"zoom":1`.
    let value = if level.fract() == 0.0 {
        serde_json::json!(level as i64)
    } else {
        serde_json::json!(level as f64)
    };
    json["zoom"] = value;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, json.to_string());
}

/// Whether `window` was asked to stay above the others.
pub fn always_on_top(window: &Window) -> bool {
    state_of(id_of(window)).is_some_and(|s| s.on_top.get())
}

/// Asks the desktop to keep `window` above the others, or to stop. The
/// flag is kept whether or not the desktop took it; the return value says
/// whether it did.
pub fn set_always_on_top(window: &Window, on: bool) -> bool {
    let Some(state) = state_of(id_of(window)) else {
        return false;
    };
    state.on_top.set(on);
    state.apply_on_top.as_ref().is_some_and(|f| f(on))
}

/// The platform gpui runs on, with every window it opens wrapped in a
/// [`ZoomWindow`]. Everything else is handed straight through.
pub struct ZoomPlatform(Rc<dyn Platform>);

impl ZoomPlatform {
    pub fn wrap(inner: Rc<dyn Platform>) -> Rc<dyn Platform> {
        Rc::new(ZoomPlatform(inner))
    }
}

impl Platform for ZoomPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.0.background_executor()
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.foreground_executor()
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.0.text_system()
    }
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        self.0.run(on_finish_launching)
    }
    fn quit(&self) {
        self.0.quit()
    }
    fn restart(&self, binary_path: Option<PathBuf>) {
        self.0.restart(binary_path)
    }
    fn activate(&self, ignoring_other_apps: bool) {
        self.0.activate(ignoring_other_apps)
    }
    fn hide(&self) {
        self.0.hide()
    }
    fn hide_other_apps(&self) {
        self.0.hide_other_apps()
    }
    fn unhide_other_apps(&self) {
        self.0.unhide_other_apps()
    }
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.0.displays()
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.0.primary_display()
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.0.active_window()
    }
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.0.window_stack()
    }
    fn is_screen_capture_supported(&self) -> bool {
        self.0.is_screen_capture_supported()
    }
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.0.screen_capture_sources()
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let inner = self.0.open_window(handle, options)?;
        let state = Rc::new(WindowState {
            apply_on_top: native_on_top(inner.as_ref()),
            ..WindowState::default()
        });
        let id = handle.window_id();
        REGISTRY.with(|r| r.borrow_mut().insert(id, state.clone()));
        Ok(Box::new(ZoomWindow { inner, state, id }))
    }
    fn window_appearance(&self) -> WindowAppearance {
        self.0.window_appearance()
    }
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.0.button_layout()
    }
    fn open_url(&self, url: &str) {
        self.0.open_url(url)
    }
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.0.on_open_urls(callback)
    }
    fn register_url_scheme(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.0.register_url_scheme(url)
    }
    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        self.0.prompt_for_paths(options)
    }
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        self.0.prompt_for_new_path(directory, suggested_name)
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.0.can_select_mixed_files_and_dirs()
    }
    fn reveal_path(&self, path: &Path) {
        self.0.reveal_path(path)
    }
    fn open_with_system(&self, path: &Path) {
        self.0.open_with_system(path)
    }
    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.0.on_quit(callback)
    }
    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.0.on_reopen(callback)
    }
    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.0.set_menus(menus, keymap)
    }
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.0.get_menus()
    }
    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.0.set_dock_menu(menu, keymap)
    }
    fn perform_dock_menu_action(&self, action: usize) {
        self.0.perform_dock_menu_action(action)
    }
    fn add_recent_document(&self, path: &Path) {
        self.0.add_recent_document(path)
    }
    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.0.update_jump_list(menus, entries)
    }
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn gpui::Action)>) {
        self.0.on_app_menu_action(callback)
    }
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.0.on_will_open_app_menu(callback)
    }
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn gpui::Action) -> bool>) {
        self.0.on_validate_app_menu_command(callback)
    }
    fn thermal_state(&self) -> ThermalState {
        self.0.thermal_state()
    }
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.0.on_thermal_state_change(callback)
    }
    fn compositor_name(&self) -> &'static str {
        self.0.compositor_name()
    }
    fn app_path(&self) -> anyhow::Result<PathBuf> {
        self.0.app_path()
    }
    fn path_for_auxiliary_executable(&self, name: &str) -> anyhow::Result<PathBuf> {
        self.0.path_for_auxiliary_executable(name)
    }
    fn set_cursor_style(&self, style: CursorStyle) {
        self.0.set_cursor_style(style)
    }
    fn hide_cursor_until_mouse_moves(&self) {
        self.0.hide_cursor_until_mouse_moves()
    }
    fn is_cursor_visible(&self) -> bool {
        self.0.is_cursor_visible()
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        self.0.should_auto_hide_scrollbars()
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.0.read_from_clipboard()
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.0.write_to_clipboard(item)
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.0.read_from_primary()
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        self.0.write_to_primary(item)
    }
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.0.read_from_find_pasteboard()
    }
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.0.write_to_find_pasteboard(item)
    }
    fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        self.0.write_credentials(url, username, password)
    }
    fn read_credentials(&self, url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        self.0.read_credentials(url)
    }
    fn delete_credentials(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.0.delete_credentials(url)
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.0.keyboard_layout()
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.0.keyboard_mapper()
    }
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.0.on_keyboard_layout_change(callback)
    }
}

/// A platform window seen through its zoom: a scale factor `factor` times
/// the display's, a content size and pointer positions `factor` times
/// smaller. At level 0 every call is the inner window's own.
struct ZoomWindow {
    inner: Box<dyn PlatformWindow>,
    state: Rc<WindowState>,
    id: WindowId,
}

impl ZoomWindow {
    fn factor(&self) -> f32 {
        factor(self.state.level.get())
    }
}

/// Logical pixels on the desktop's side (`physical / display scale`) to
/// the zoomed window's (`/ factor`), and back.
fn shrink(p: Pixels, f: f32) -> Pixels {
    p / f
}
fn grow(p: Pixels, f: f32) -> Pixels {
    p * f
}
fn shrink_point(p: Point<Pixels>, f: f32) -> Point<Pixels> {
    Point::new(shrink(p.x, f), shrink(p.y, f))
}
fn grow_bounds(b: Bounds<Pixels>, f: f32) -> Bounds<Pixels> {
    Bounds::new(
        Point::new(grow(b.origin.x, f), grow(b.origin.y, f)),
        Size::new(grow(b.size.width, f), grow(b.size.height, f)),
    )
}

/// An input event's positions (and a pixel scroll's delta) in the zoomed
/// window's pixels.
fn zoom_input(event: PlatformInput, f: f32) -> PlatformInput {
    if f == 1.0 {
        return event;
    }
    match event {
        PlatformInput::MouseDown(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::MouseDown(e)
        }
        PlatformInput::MouseUp(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::MouseUp(e)
        }
        PlatformInput::MousePressure(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::MousePressure(e)
        }
        PlatformInput::MouseMove(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::MouseMove(e)
        }
        PlatformInput::MouseExited(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::MouseExited(e)
        }
        PlatformInput::ScrollWheel(mut e) => {
            e.position = shrink_point(e.position, f);
            if let ScrollDelta::Pixels(d) = e.delta {
                e.delta = ScrollDelta::Pixels(shrink_point(d, f));
            }
            PlatformInput::ScrollWheel(e)
        }
        PlatformInput::Pinch(mut e) => {
            e.position = shrink_point(e.position, f);
            PlatformInput::Pinch(e)
        }
        PlatformInput::FileDrop(drop) => PlatformInput::FileDrop(match drop {
            FileDropEvent::Entered { position, paths } => FileDropEvent::Entered {
                position: shrink_point(position, f),
                paths,
            },
            FileDropEvent::Pending { position } => FileDropEvent::Pending {
                position: shrink_point(position, f),
            },
            FileDropEvent::Submit { position } => FileDropEvent::Submit {
                position: shrink_point(position, f),
            },
            FileDropEvent::Exited => FileDropEvent::Exited,
        }),
        other => other,
    }
}

impl HasWindowHandle for ZoomWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for ZoomWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

impl PlatformWindow for ZoomWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.inner.bounds()
    }
    fn is_maximized(&self) -> bool {
        self.inner.is_maximized()
    }
    fn window_bounds(&self) -> WindowBounds {
        self.inner.window_bounds()
    }
    fn content_size(&self) -> Size<Pixels> {
        let f = self.factor();
        let s = self.inner.content_size();
        Size::new(shrink(s.width, f), shrink(s.height, f))
    }
    fn resize(&mut self, size: Size<Pixels>) {
        let f = self.factor();
        self.inner
            .resize(Size::new(grow(size.width, f), grow(size.height, f)))
    }
    fn scale_factor(&self) -> f32 {
        self.inner.scale_factor() * self.factor()
    }
    fn appearance(&self) -> WindowAppearance {
        self.inner.appearance()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.inner.display()
    }
    fn mouse_position(&self) -> Point<Pixels> {
        shrink_point(self.inner.mouse_position(), self.factor())
    }
    fn modifiers(&self) -> Modifiers {
        self.inner.modifiers()
    }
    fn capslock(&self) -> Capslock {
        self.inner.capslock()
    }
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.inner.set_input_handler(input_handler)
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.inner.take_input_handler()
    }
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        self.inner.prompt(level, msg, detail, answers)
    }
    fn activate(&self) {
        self.inner.activate()
    }
    fn is_active(&self) -> bool {
        self.inner.is_active()
    }
    fn is_hovered(&self) -> bool {
        self.inner.is_hovered()
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.inner.background_appearance()
    }
    fn set_title(&mut self, title: &str) {
        self.inner.set_title(title)
    }
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.inner.set_background_appearance(background_appearance)
    }
    fn minimize(&self) {
        self.inner.minimize()
    }
    fn zoom(&self) {
        self.inner.zoom()
    }
    fn toggle_fullscreen(&self) {
        self.inner.toggle_fullscreen()
    }
    fn is_fullscreen(&self) -> bool {
        self.inner.is_fullscreen()
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.inner.on_request_frame(callback)
    }
    fn on_input(&self, mut callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        let state = self.state.clone();
        self.inner.on_input(Box::new(move |event| {
            callback(zoom_input(event, factor(state.level.get())))
        }))
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.inner.on_active_status_change(callback)
    }
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.inner.on_hover_status_change(callback)
    }
    fn on_resize(&self, mut callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        let state = self.state.clone();
        self.inner.on_resize(Box::new(move |size, scale| {
            let f = factor(state.level.get());
            callback(
                Size::new(shrink(size.width, f), shrink(size.height, f)),
                scale * f,
            )
        }))
    }
    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_moved(callback)
    }
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.inner.on_should_close(callback)
    }
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.inner.on_hit_test_window_control(callback)
    }
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        let id = self.id;
        self.inner.on_close(Box::new(move || {
            REGISTRY.with(|r| r.borrow_mut().remove(&id));
            callback()
        }))
    }
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_appearance_changed(callback)
    }
    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_button_layout_changed(callback)
    }
    fn draw(&self, scene: &Scene) {
        self.inner.draw(scene)
    }
    fn completed_frame(&self) {
        self.inner.completed_frame()
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.inner.sprite_atlas()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        self.inner.is_subpixel_rendering_supported()
    }
    fn get_title(&self) -> String {
        self.inner.get_title()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.inner.tabbed_windows()
    }
    fn tab_bar_visible(&self) -> bool {
        self.inner.tab_bar_visible()
    }
    fn set_edited(&mut self, edited: bool) {
        self.inner.set_edited(edited)
    }
    fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.inner.set_document_path(path)
    }
    fn show_character_palette(&self) {
        self.inner.show_character_palette()
    }
    fn titlebar_double_click(&self) {
        self.inner.titlebar_double_click()
    }
    fn on_move_tab_to_new_window(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_move_tab_to_new_window(callback)
    }
    fn on_merge_all_windows(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_merge_all_windows(callback)
    }
    fn on_select_previous_tab(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_select_previous_tab(callback)
    }
    fn on_select_next_tab(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_select_next_tab(callback)
    }
    fn on_toggle_tab_bar(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_toggle_tab_bar(callback)
    }
    fn merge_all_windows(&self) {
        self.inner.merge_all_windows()
    }
    fn move_tab_to_new_window(&self) {
        self.inner.move_tab_to_new_window()
    }
    fn toggle_window_tab_overview(&self) {
        self.inner.toggle_window_tab_overview()
    }
    fn set_tabbing_identifier(&self, identifier: Option<String>) {
        self.inner.set_tabbing_identifier(identifier)
    }
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        self.inner.get_raw_handle()
    }
    fn inner_window_bounds(&self) -> WindowBounds {
        self.inner.inner_window_bounds()
    }
    fn request_decorations(&self, decorations: WindowDecorations) {
        self.inner.request_decorations(decorations)
    }
    fn show_window_menu(&self, position: Point<Pixels>) {
        let f = self.factor();
        self.inner
            .show_window_menu(Point::new(grow(position.x, f), grow(position.y, f)))
    }
    fn start_window_move(&self) {
        self.inner.start_window_move()
    }
    fn start_window_resize(&self, edge: ResizeEdge) {
        self.inner.start_window_resize(edge)
    }
    fn window_decorations(&self) -> Decorations {
        self.inner.window_decorations()
    }
    fn set_app_id(&mut self, app_id: &str) {
        self.inner.set_app_id(app_id)
    }
    fn map_window(&mut self) -> anyhow::Result<()> {
        self.inner.map_window()
    }
    fn window_controls(&self) -> WindowControls {
        self.inner.window_controls()
    }
    fn set_client_inset(&self, inset: Pixels) {
        self.inner.set_client_inset(grow(inset, self.factor()))
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.inner.gpu_specs()
    }
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.inner
            .update_ime_position(grow_bounds(bounds, self.factor()))
    }
    fn play_system_bell(&self) {
        self.inner.play_system_bell()
    }
}

/// The desktop's own "keep this window above the others" for `window`, or
/// `None` where there is none to call. Each takes the native handle gpui's
/// window holds; the call itself happens when the command runs.
fn native_on_top(window: &dyn PlatformWindow) -> Option<OnTop> {
    use raw_window_handle::RawWindowHandle;
    let raw = window.window_handle().ok()?.as_raw();
    match raw {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        RawWindowHandle::Xlib(h) => {
            let id = h.window as u32;
            Some(Box::new(move |on| x11_on_top(id, on).is_ok()))
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        RawWindowHandle::Xcb(h) => {
            let id = h.window.get();
            Some(Box::new(move |on| x11_on_top(id, on).is_ok()))
        }
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(h) => {
            let view = h.ns_view.as_ptr() as usize;
            Some(Box::new(move |on| macos_on_top(view, on)))
        }
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(h) => {
            let hwnd = h.hwnd.get();
            Some(Box::new(move |on| windows_on_top(hwnd, on)))
        }
        // Wayland: xdg-shell has no request for it, by design; the
        // compositor alone decides what stays on top.
        _ => None,
    }
}

/// EWMH's way: a `_NET_WM_STATE` client message to the root window asking
/// the window manager to add or remove `_NET_WM_STATE_ABOVE`, on a
/// connection of its own (gpui's is private to it).
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn x11_on_top(window: u32, on: bool) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt as _, EventMask};
    let (conn, screen) = x11rb::rust_connection::RustConnection::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let above = conn
        .intern_atom(false, b"_NET_WM_STATE_ABOVE")?
        .reply()?
        .atom;
    // 1 adds, 0 removes; the last word is the source, 1 for an application.
    let action = if on { 1 } else { 0 };
    let event = ClientMessageEvent::new(32, window, state, [action, above, 0, 1, 0]);
    // Checked, which is a round trip: a connection closed straight after
    // its flush lost about one message in three under openbox.
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )?
    .check()?;
    Ok(())
}

/// AppKit's way: the window's level, `NSFloatingWindowLevel` (3) above the
/// normal windows and `NSNormalWindowLevel` (0) back, which is what
/// Electron's `setAlwaysOnTop(true)` sets by default.
#[cfg(target_os = "macos")]
fn macos_on_top(view: usize, on: bool) -> bool {
    use objc2::runtime::AnyObject;
    let view = view as *mut AnyObject;
    if view.is_null() {
        return false;
    }
    // SAFETY: the pointer is the live NSView gpui made for this window and
    // the call is on the main thread, where gpui runs.
    unsafe {
        let window: *mut AnyObject = objc2::msg_send![view, window];
        if window.is_null() {
            return false;
        }
        let level: isize = if on { 3 } else { 0 };
        let _: () = objc2::msg_send![window, setLevel: level];
    }
    true
}

/// Win32's way: `SetWindowPos` with `HWND_TOPMOST`, or `HWND_NOTOPMOST`
/// back, leaving the size and place alone.
#[cfg(target_os = "windows")]
fn windows_on_top(hwnd: isize, on: bool) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    // SAFETY: the handle is the live window gpui made for this window.
    unsafe {
        SetWindowPos(
            HWND(hwnd as *mut _),
            Some(after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_factors_are_chromiums() {
        // The table of [[Specification#Window zoom]], to its four places.
        for (level, want) in [
            (3.0, 1.728),
            (2.5, 1.5774),
            (2.0, 1.44),
            (1.5, 1.3145),
            (1.0, 1.2),
            (0.5, 1.0954),
            (0.0, 1.0),
            (-0.5, 0.9129),
            (-1.0, 0.8333),
            (-1.5, 0.7607),
            (-2.0, 0.6944),
            (-2.5, 0.6339),
        ] {
            assert!(
                (factor(level) - want).abs() < 0.0001,
                "{level}: {}",
                factor(level)
            );
        }
    }

    #[test]
    fn steps_are_halves_held_inside_the_limits() {
        // Twenty Zoom ins from 0 stop at 3 on the sixth; forty Zoom outs
        // from 3 stop at -2.5 on the eleventh.
        let mut level = 0.0;
        let mut ins = vec![];
        for _ in 0..20 {
            level = stepped(level, 1);
            ins.push(level);
        }
        assert_eq!(&ins[..7], &[0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0]);
        assert!(ins.iter().all(|l| *l <= MAX_LEVEL));
        let mut outs = vec![];
        for _ in 0..40 {
            level = stepped(level, -1);
            outs.push(level);
        }
        assert_eq!(outs[10], -2.5);
        assert_eq!(outs[9], -2.0);
        assert!(outs[10..].iter().all(|l| *l == MIN_LEVEL));
        // A level read from a file off the half grid snaps back to it.
        assert_eq!(stepped(0.4999, 1), 1.0);
    }

    #[test]
    fn the_level_is_kept_in_a_window_file_of_scorias_own() {
        let config = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        let path = window_file_in(config.path(), vault.path()).unwrap();
        assert!(path.starts_with(config.path().join("window")));
        assert_eq!(read_level(&path), 0.0, "no file is level 0");
        // Another key the file holds stays; a whole level is an integer,
        // the way Obsidian's `"zoom":1` is.
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"x":10,"zoom":0}"#).unwrap();
        write_level(&path, 1.0);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"x":10,"zoom":1}"#
        );
        write_level(&path, -1.5);
        assert_eq!(read_level(&path), -1.5);
        // A level past the limits, written by hand, is held to them.
        std::fs::write(&path, r#"{"zoom":9}"#).unwrap();
        assert_eq!(read_level(&path), MAX_LEVEL);
        // Nothing went into the vault.
        assert_eq!(std::fs::read_dir(vault.path()).unwrap().count(), 0);
    }

    #[test]
    fn input_positions_shrink_by_the_factor() {
        let f = factor(1.0);
        let e = PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: Point::new(gpui::px(120.), gpui::px(60.)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        });
        let PlatformInput::MouseMove(m) = zoom_input(e, f) else {
            panic!("not a move");
        };
        assert!((m.position.x.as_f32() - 100.).abs() < 0.001);
        assert!((m.position.y.as_f32() - 50.).abs() < 0.001);
    }
}
