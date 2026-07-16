//! The persistent dictation-orb webview window (Windows/Linux).
//!
//! Follows the same creation pattern as the meeting floating bar
//! (`plugins/windows/src/window/floating_bar.rs`): transparent always-on-top
//! webview with a solid fallback (`?solid=1`) for environments where a
//! transparent window cannot be created, plus a materialization probe so a
//! failed OS-side creation surfaces as an error instead of a ghost window.
//!
//! One critical difference: the orb is created with `focusable(false)` so
//! clicking it never steals keyboard focus from the app being dictated into —
//! otherwise the injected text would lose its target.

use std::sync::OnceLock;

use tauri::Manager;

use crate::error::Error;

pub const WINDOW_LABEL: &str = "dictation";

const ORB_SIZE: f64 = 56.0;
const BOTTOM_MARGIN: f64 = 32.0;

/// Route served by the SPA for the orb window. Must match a route in
/// `apps/desktop/src/routeTree.gen.ts` (`/app/dictation`).
const WINDOW_URL: &str = "/app/dictation";
/// Same route, telling the webview to render its solid-surface variant.
const SOLID_WINDOW_URL: &str = "/app/dictation?solid=1";

/// `bg` token of the dark theme (`#0B0D12`, docs/DESIGN-DIRECTION.md §2),
/// used as the window background of the solid fallback.
const SOLID_BACKGROUND: tauri::window::Color = tauri::window::Color(0x0B, 0x0D, 0x12, 0xFF);

static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();

pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
    let _ = APP_HANDLE.set(app);
}

pub fn app_handle() -> Result<&'static tauri::AppHandle<tauri::Wry>, Error> {
    APP_HANDLE
        .get()
        .ok_or_else(|| Error::OrbWindow("dictation orb app handle is not initialized".to_string()))
}

pub fn show() -> Result<(), Error> {
    let app = app_handle()?;
    let window = get_or_create_window(app)?;
    window.show().map_err(|error| {
        tracing::error!(%error, label = WINDOW_LABEL, "failed to show dictation orb window");
        Error::OrbWindow(error.to_string())
    })?;
    let _ = window.set_always_on_top(true);
    Ok(())
}

pub fn hide() -> Result<(), Error> {
    let app = app_handle()?;
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| Error::OrbWindow(error.to_string()))?;
    }
    Ok(())
}

fn get_or_create_window(
    app: &tauri::AppHandle<tauri::Wry>,
) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        // A window whose OS-side creation failed earlier stays registered in
        // the manager as a "ghost": commands against it are silently dropped.
        // Probe it through the event loop; recreate if it has no OS backing.
        match window.is_visible() {
            Ok(_) => return Ok(window),
            Err(error) => {
                tracing::warn!(
                    %error,
                    label = WINDOW_LABEL,
                    "existing dictation orb window is not backed by an OS \
                     window; destroying the ghost and recreating"
                );
                destroy_and_wait_unregistered(app, &window);
            }
        }
    }

    let window = match build_window(app, true) {
        Ok(window) => window,
        Err(error) => {
            // Known failure mode on Windows: transparent window creation can
            // fail depending on the WebView2/compositor environment. Fall back
            // to an opaque window rendering the solid variant.
            tracing::warn!(
                %error,
                label = WINDOW_LABEL,
                "transparent dictation orb window failed to create; retrying \
                 with a solid background"
            );
            if let Some(ghost) = app.get_webview_window(WINDOW_LABEL) {
                destroy_and_wait_unregistered(app, &ghost);
            }
            build_window(app, false)?
        }
    };

    position_bottom_center(app, &window);

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
        // Never activate on click: keyboard focus must stay on the app that
        // receives the dictated text (WS_EX_NOACTIVATE on Windows).
        .focusable(false)
        .visible(false)
        .inner_size(ORB_SIZE, ORB_SIZE)
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
            "failed to build dictation orb window"
        );
        Error::OrbWindow(format!(
            "failed to create dictation orb window `{WINDOW_LABEL}` (url `{url}`): {error}"
        ))
    })?;

    // When `build()` runs off the main thread (async command), the actual OS
    // window/webview creation is queued onto the event loop and its errors are
    // only ever logged by `tauri_runtime_wry` - `build()` still returns `Ok`.
    // Round-trip a getter through the event loop so a failed creation surfaces
    // here as an error instead of an invisible window.
    if let Err(error) = window.is_visible() {
        tracing::error!(
            %error,
            label = WINDOW_LABEL,
            url,
            transparent,
            "dictation orb window did not materialize after build; check the \
             log for `tauri_runtime_wry` window/webview creation errors"
        );
        let _ = window.destroy();
        return Err(Error::OrbWindow(format!(
            "dictation orb window `{WINDOW_LABEL}` was not created by the OS event loop: {error}"
        )));
    }

    tracing::info!(
        label = WINDOW_LABEL,
        url,
        transparent,
        "created dictation orb window"
    );

    Ok(window)
}

/// `destroy()` is dispatched through the event loop, so the label can still be
/// registered for a short while afterwards; wait it out so an immediate
/// rebuild with the same label does not collide.
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
        "dictation orb window still registered after destroy; the rebuild may \
         fail with a duplicate-label error"
    );
}

fn position_bottom_center(
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
    let x = position.x + ((size.width - ORB_SIZE) / 2.0);
    let y = position.y + size.height - ORB_SIZE - BOTTOM_MARGIN;
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}
