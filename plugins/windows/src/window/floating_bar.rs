use serde::{Deserialize, Serialize};

use crate::Error;
use crate::window::live_caption::LiveCaptionPosition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FloatingBarStatus {
    Recording,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FloatingBarColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingTranscriptBubble {
    pub id: String,
    pub speaker_label: String,
    pub text: String,
    pub is_self: bool,
    pub is_final: bool,
    pub start_ms: f64,
    pub end_ms: f64,
    pub overlaps_previous: bool,
    pub overlaps_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarState {
    pub amplitude: f64,
    pub title: String,
    pub status: FloatingBarStatus,
    pub color_scheme: FloatingBarColorScheme,
    pub opacity: f64,
    pub live_caption_opacity: f64,
    pub live_caption_width: f64,
    pub live_caption_line_count: u32,
    pub live_caption_position: LiveCaptionPosition,
    pub live_caption_minimized: bool,
    pub live_caption_toggle_visible: bool,
    pub transcript_bubbles: Vec<FloatingTranscriptBubble>,
}

/// Native macOS floating bar (Swift NSPanel via `FloatingBarManager`).
///
/// Gated off since Track C: macOS now uses the SAME webview floating bar as
/// Windows/Linux (`mod platform` below), so the Swift FFI path is never
/// reached - nothing calls `_floating_bar_show`, so the NSPanel never appears
/// and the `rust_on_floating_bar_*` callbacks never fire (their `APP_HANDLE`
/// stays empty, since `pub use platform::set_app_handle` now points at the
/// webview module). Kept in place rather than deleted, exactly like #31 did
/// for the dictation shortcut→native-panel bridge: the `swift-lib` symbols
/// still link, and re-enabling the native panel would only need this module
/// wired back into `pub use`. Do NOT add a second `pub use` here.
#[cfg(target_os = "macos")]
mod macos_native_swift {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    use swift_rs::{Bool, SRString, swift};
    use tauri_specta::Event;

    use super::FloatingBarState;
    use crate::Error;

    swift!(fn _floating_bar_show() -> Bool);
    swift!(fn _floating_bar_hide() -> Bool);
    swift!(fn _floating_bar_update(json: &SRString) -> Bool);

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    pub fn show() -> Result<(), Error> {
        unsafe {
            _floating_bar_show();
        }
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _floating_bar_hide();
        }
        Ok(())
    }

    pub fn update(state: FloatingBarState) -> Result<(), Error> {
        let json = serde_json::to_string(&state).map_err(|error| {
            Error::PanelError(format!("failed to serialize floating bar state: {error}"))
        })?;
        let json = SRString::from(json.as_str());

        let ok = unsafe { _floating_bar_update(&json) };
        if ok {
            Ok(())
        } else {
            Err(Error::PanelError(
                "failed to update native floating bar".to_string(),
            ))
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_stop() {
        if let Some(app) = APP_HANDLE.get() {
            let _ = crate::events::FloatingBarStop {}.emit(app);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_open_main() {
        if let Some(app) = APP_HANDLE.get() {
            let _ = crate::events::FloatingBarOpenMain {}.emit(app);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_settings_change(settings_ptr: *const c_char) {
        if settings_ptr.is_null() {
            return;
        }

        let Ok(settings_json) = (unsafe { CStr::from_ptr(settings_ptr) }).to_str() else {
            return;
        };

        let Ok(settings) =
            serde_json::from_str::<crate::events::FloatingBarSettingsChange>(settings_json)
        else {
            return;
        };

        if let Some(app) = APP_HANDLE.get() {
            let _ = settings.emit(app);
        }
    }
}

/// Webview-based floating bar, used on EVERY platform since Track C (macOS
/// previously rendered the Swift NSPanel in `macos_native_swift` above; that
/// path is now gated off). A small always-on-top transparent webview window
/// pointing at `/app/floating` that streams `FloatingBarStateEvent`s; the
/// React side (`apps/desktop/src/meeting-float/window.tsx`) renders either the
/// Notare glass bar or the Classic parchment bar based on the
/// `meeting_bar_theme` setting.
mod platform {
    use std::sync::OnceLock;

    use tauri::Manager;
    use tauri_specta::Event;

    use super::FloatingBarState;
    use crate::Error;

    pub const WINDOW_LABEL: &str = "floating";

    const BAR_WIDTH: f64 = 380.0;
    const BAR_HEIGHT: f64 = 52.0;
    const CAPTION_LINE_HEIGHT: f64 = 22.0;
    const CAPTION_VERTICAL_PADDING: f64 = 26.0;
    const MAX_CAPTION_WIDTH: f64 = 640.0;
    const MAX_CAPTION_LINE_COUNT: u32 = 4;

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    fn app_handle() -> Result<&'static tauri::AppHandle<tauri::Wry>, Error> {
        APP_HANDLE.get().ok_or_else(|| {
            Error::PanelError("floating bar app handle is not initialized".to_string())
        })
    }

    fn target_size(state: &FloatingBarState) -> (f64, f64) {
        if state.live_caption_minimized {
            return (BAR_WIDTH, BAR_HEIGHT);
        }

        let width = state.live_caption_width.clamp(BAR_WIDTH, MAX_CAPTION_WIDTH);
        let line_count = f64::from(
            state
                .live_caption_line_count
                .clamp(1, MAX_CAPTION_LINE_COUNT),
        );
        let caption_height = (line_count * CAPTION_LINE_HEIGHT) + CAPTION_VERTICAL_PADDING;

        (width, BAR_HEIGHT + caption_height)
    }

    /// Route served by the SPA for the floating bar window. Must match a
    /// route in `apps/desktop/src/routeTree.gen.ts` (`/app/floating`), same
    /// `WebviewUrl::App` pattern the v1 windows use (e.g. `/app`).
    const WINDOW_URL: &str = "/app/floating";
    /// Same route, telling the webview to render its solid-surface variant
    /// (used when the OS refuses a transparent window; see
    /// `get_or_create_window`).
    const SOLID_WINDOW_URL: &str = "/app/floating?solid=1";

    /// `bg` token of the dark theme (`#0B0D12`, docs/DESIGN-DIRECTION.md §2),
    /// used as the window background of the solid fallback so the corners
    /// blend with the widget surface.
    const SOLID_BACKGROUND: tauri::window::Color = tauri::window::Color(0x0B, 0x0D, 0x12, 0xFF);

    fn get_or_create_window(
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            // A window whose OS-side creation failed earlier stays registered
            // in the manager as a "ghost": commands against it are silently
            // dropped, so the bar would never appear and never error. Probe it
            // through the event loop; if it is not backed by an OS window,
            // destroy the ghost and recreate instead of failing until the app
            // restarts.
            match window.is_visible() {
                Ok(_) => return Ok(window),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        label = WINDOW_LABEL,
                        "existing floating bar window is not backed by an OS window \
                         (earlier creation failed; see `tauri_runtime_wry` errors in \
                         the log); destroying the ghost and recreating"
                    );
                    destroy_and_wait_unregistered(app, &window);
                }
            }
        }

        let window = match build_window(app, true) {
            Ok(window) => window,
            Err(error) => {
                // Known failure mode on Windows: transparent window creation
                // can fail depending on the WebView2/compositor environment.
                // Fall back to an opaque window and let the webview render
                // the solid variant (`?solid=1`).
                tracing::warn!(
                    %error,
                    label = WINDOW_LABEL,
                    "transparent floating bar window failed to create; \
                     retrying with a solid background"
                );
                if let Some(ghost) = app.get_webview_window(WINDOW_LABEL) {
                    destroy_and_wait_unregistered(app, &ghost);
                }
                build_window(app, false)?
            }
        };

        position_top_center(app, &window);
        apply_macos_panel_traits(&window);

        Ok(window)
    }

    fn build_window(
        app: &tauri::AppHandle<tauri::Wry>,
        transparent: bool,
    ) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
        use tauri::{WebviewUrl, WebviewWindow};

        let url = if transparent {
            WINDOW_URL
        } else {
            SOLID_WINDOW_URL
        };

        let mut builder = WebviewWindow::builder(app, WINDOW_LABEL, WebviewUrl::App(url.into()))
            .title(
                app.config()
                    .product_name
                    .clone()
                    .unwrap_or_else(|| "Notare".to_string()),
            )
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .shadow(false)
            .transparent(transparent)
            .focused(false)
            // Non-activating panel parity: clicking the bar never steals
            // keyboard focus (the native NSPanel used `.nonactivatingPanel`).
            // Also matches the dictation orb (`plugins/dictation/src/orb.rs`).
            .focusable(false)
            // A non-activating window on macOS never becomes key, so AppKit
            // swallows the first left-click instead of delivering it to the
            // WKWebView - the bar's buttons (stop, captions, open) would not
            // respond to a click. `accept_first_mouse` (default false) passes
            // the click through to the webview without taking key focus. No-op
            // on Windows/Linux. Mirrors the fix in the dictation orb.
            .accept_first_mouse(true)
            // macOS `sharingType = .none` parity: the window contents are
            // excluded from screen-sharing captures. No-op on Windows/Linux.
            .content_protected(true)
            .visible(false)
            .inner_size(BAR_WIDTH, BAR_HEIGHT)
            .disable_drag_drop_handler();
        if !transparent {
            builder = builder.background_color(SOLID_BACKGROUND);
        }

        let window = builder.build().map_err(|error| {
            tracing::error!(
                %error,
                label = WINDOW_LABEL,
                url,
                transparent,
                "failed to build floating bar window"
            );
            Error::PanelError(format!(
                "failed to create floating bar window `{WINDOW_LABEL}` \
                 (url `{url}`): {error}"
            ))
        })?;

        // When `build()` runs off the main thread (async command), the actual
        // OS window/webview creation is queued onto the event loop and its
        // errors are only ever logged by `tauri_runtime_wry` - `build()` still
        // returns `Ok`. Round-trip a getter through the event loop so a failed
        // creation surfaces here as an error instead of an invisible window.
        if let Err(error) = window.is_visible() {
            tracing::error!(
                %error,
                label = WINDOW_LABEL,
                url,
                transparent,
                "floating bar window did not materialize after build; check \
                 the log for `tauri_runtime_wry` window/webview creation errors"
            );
            let _ = window.destroy();
            return Err(Error::PanelError(format!(
                "floating bar window `{WINDOW_LABEL}` was not created by the \
                 OS event loop: {error}"
            )));
        }

        tracing::info!(
            label = WINDOW_LABEL,
            url,
            transparent,
            "created floating bar window"
        );

        Ok(window)
    }

    /// Best-effort macOS-only window tweaks that replicate the native NSPanel
    /// (`FloatingBarManager.swift`) traits the webview builder cannot express:
    /// OR `FullScreenAuxiliary | Stationary` into the window's
    /// `collectionBehavior`. `visible_on_all_workspaces(true)` already set
    /// `CanJoinAllSpaces`; `FullScreenAuxiliary` lets the bar show over
    /// full-screen apps and `Stationary` keeps it from drifting with Space
    /// switches (the native panel used both). No-op off macOS.
    ///
    /// VERIFY on macOS CI / the user's Mac (this box is Linux, so the objc2
    /// path is not compiled here): the bar should stay visible in fullscreen
    /// apps and across Space switches, and never join a Space.
    #[cfg(target_os = "macos")]
    fn apply_macos_panel_traits(window: &tauri::WebviewWindow<tauri::Wry>) {
        use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

        let app = window.app_handle().clone();
        let win = window.clone();
        let result = crate::ext::run_on_main_thread(&app, move || {
            let Ok(ptr) = win.ns_window() else {
                return;
            };

            // SAFETY: `ns_window()` returns the underlying NSWindow owned by
            // the still-alive tauri window; `run_on_main_thread` guarantees
            // AppKit main-thread access.
            unsafe {
                let ns_window = &*(ptr as *mut NSWindow);
                let mut behavior = ns_window.collectionBehavior();
                behavior |= NSWindowCollectionBehavior::FullScreenAuxiliary;
                behavior |= NSWindowCollectionBehavior::Stationary;
                ns_window.setCollectionBehavior(behavior);
            }
        });

        if let Err(error) = result {
            tracing::warn!(
                %error,
                label = WINDOW_LABEL,
                "failed to apply macOS collectionBehavior bits to the floating bar"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn apply_macos_panel_traits(_window: &tauri::WebviewWindow<tauri::Wry>) {}

    /// `destroy()` is dispatched through the event loop, so the label can
    /// still be registered for a short while afterwards; wait it out so an
    /// immediate rebuild with the same label does not collide.
    fn destroy_and_wait_unregistered(
        app: &tauri::AppHandle<tauri::Wry>,
        window: &tauri::WebviewWindow<tauri::Wry>,
    ) {
        let _ = window.destroy();

        for _ in 0..25 {
            if app.get_webview_window(WINDOW_LABEL).is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        tracing::warn!(
            label = WINDOW_LABEL,
            "floating bar window still registered after destroy; the rebuild \
             may fail with a duplicate-label error"
        );
    }

    fn position_top_center(
        app: &tauri::AppHandle<tauri::Wry>,
        window: &tauri::WebviewWindow<tauri::Wry>,
    ) {
        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else {
            return;
        };

        let scale = monitor.scale_factor();
        let position = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        let x = position.x + ((size.width - BAR_WIDTH) / 2.0);
        let y = position.y + 24.0;
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    }

    pub fn show() -> Result<(), Error> {
        let app = app_handle()?;
        let window = get_or_create_window(app)?;
        window.show().map_err(|error| {
            tracing::error!(%error, label = WINDOW_LABEL, "failed to show floating bar window");
            Error::TauriError(error)
        })?;
        let _ = window.set_always_on_top(true);
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        let app = app_handle()?;
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window.hide()?;
        }
        Ok(())
    }

    pub fn update(state: FloatingBarState) -> Result<(), Error> {
        let app = app_handle()?;
        let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
            return Ok(());
        };

        let (width, height) = target_size(&state);
        let needs_resize = match (window.scale_factor(), window.inner_size()) {
            (Ok(scale), Ok(size)) => {
                let size = size.to_logical::<f64>(scale);
                (size.width - width).abs() > 1.0 || (size.height - height).abs() > 1.0
            }
            _ => true,
        };
        if needs_resize {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
        }

        crate::events::FloatingBarStateEvent { state }
            .emit(app)
            .map_err(|error| {
                tracing::error!(%error, "failed to emit floating bar state event");
                Error::TauriError(error)
            })?;

        Ok(())
    }
}

pub use platform::set_app_handle;

pub fn show() -> Result<(), Error> {
    platform::show()
}

pub fn hide() -> Result<(), Error> {
    platform::hide()
}

pub fn update(state: FloatingBarState) -> Result<(), Error> {
    platform::update(state)
}
