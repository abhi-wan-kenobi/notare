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

#[cfg(target_os = "macos")]
mod platform {
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

#[cfg(not(target_os = "macos"))]
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

        let width = state
            .live_caption_width
            .clamp(BAR_WIDTH, MAX_CAPTION_WIDTH);
        let line_count = f64::from(state.live_caption_line_count.clamp(1, MAX_CAPTION_LINE_COUNT));
        let caption_height = (line_count * CAPTION_LINE_HEIGHT) + CAPTION_VERTICAL_PADDING;

        (width, BAR_HEIGHT + caption_height)
    }

    fn get_or_create_window(
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
        use tauri::{WebviewUrl, WebviewWindow};

        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            return Ok(window);
        }

        let window = WebviewWindow::builder(
            app,
            WINDOW_LABEL,
            WebviewUrl::App("/app/floating".into()),
        )
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
        .transparent(true)
        .focused(false)
        .visible(false)
        .inner_size(BAR_WIDTH, BAR_HEIGHT)
        .disable_drag_drop_handler()
        .build()?;

        position_top_center(app, &window);

        Ok(window)
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
        window.show()?;
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
            .map_err(Error::TauriError)?;

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
