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
//!
//! The orb is draggable (the webview calls `startDragging()` past a small
//! pointer threshold); its position is persisted to the store2 store on move
//! (debounced) and restored - clamped to a visible monitor - on creation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_store2::Store2PluginExt;

#[cfg(target_os = "macos")]
use tauri_nspanel::tauri_panel;

use crate::error::Error;

pub const WINDOW_LABEL: &str = "dictation";

/// BUG 1 fix (macOS): the orb is an `NSPanel` with the `.nonactivatingPanel`
/// style mask, NOT a plain `NSWindow`. `.focusable(false)` on a plain
/// `NSWindow` does not stop AppKit's "click a background app's window ->
/// activate the app" behavior, so every orb click raised the app. A real
/// `NSPanel` styled `nonactivatingPanel` receives the click without
/// activating. The orb must NOT become key (it is non-focusable, unlike
/// `plugins/windows/src/window/composer.rs::ComposerPanel` which sets
/// `can_become_key_window: true`), so it never steals keyboard focus from
/// the app receiving the dictated text.
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(OrbPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true,
        }
    })
}

/// Logical window size the orb is CREATED at (the cobalt-variant chassis).
/// The orb webview resizes the window per orb variant once it knows the
/// `dictation_orb_variant` setting (`syncOrbWindowSize` in
/// `apps/desktop/src/dictation/window.tsx`), so treat this as the default
/// size, not an invariant.
const ORB_SIZE: f64 = 70.0;
const BOTTOM_MARGIN: f64 = 32.0;

/// The live-caption window that floats just above the orb: label, logical
/// size and the gap to the orb. It is a SEPARATE window (instead of an
/// enlarged orb window) because wry has no per-pixel hit testing - a bigger
/// transparent orb window would swallow clicks around the orb. The caption
/// window is marked `set_ignore_cursor_events(true)`, so it can never
/// intercept anything; its webview shows/hides it around live text
/// (`apps/desktop/src/dictation/caption.tsx`).
pub const CAPTION_WINDOW_LABEL: &str = "dictation-caption";
const CAPTION_WIDTH: f64 = 320.0;
const CAPTION_HEIGHT: f64 = 84.0;
const CAPTION_GAP: f64 = 10.0;

/// store2 scope + key holding the persisted orb position (logical pixels).
const STORE_SCOPE: &str = "dictation";
const STORE_KEY_ORB_POSITION: &str = "orb_position";
/// Move events stream continuously during a drag; writes are coalesced.
const POSITION_SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct OrbPosition {
    x: f64,
    y: f64,
}

/// Latest not-yet-persisted position + whether a save task is running.
static PENDING_POSITION: Mutex<Option<OrbPosition>> = Mutex::new(None);
static SAVE_TASK_RUNNING: AtomicBool = AtomicBool::new(false);

/// Route served by the SPA for the orb window. Must match a route in
/// `apps/desktop/src/routeTree.gen.ts` (`/app/dictation`).
const WINDOW_URL: &str = "/app/dictation";
/// Same route, telling the webview to render its solid-surface variant.
const SOLID_WINDOW_URL: &str = "/app/dictation?solid=1";
/// Same route rendering the live-caption variant (second webview).
const CAPTION_WINDOW_URL: &str = "/app/dictation?caption=1";
/// Caption variant on an opaque window (transparency fallback).
const CAPTION_SOLID_WINDOW_URL: &str = "/app/dictation?caption=1&solid=1";

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

    // BUG 2 fix: re-clamp on every show so a monitor unplug/dock/DPI change
    // that left the persisted position off-screen self-heals here instead of
    // needing an app restart. `restore_position` previously ran only on the
    // fresh-build path (`get_or_create_window`), so a reused orb kept stale,
    // now-off-screen coordinates forever.
    restore_position(app, &window);

    #[cfg(target_os = "macos")]
    {
        // BUG 1 fix: show through the NSPanel handle, not the plain webview
        // window, so the non-activating panel appears without raising the app.
        // Use `show()` (not `show_and_make_key()`): the orb must never become
        // key (it is non-focusable).
        use tauri_nspanel::ManagerExt;

        let app_clone = app.clone();
        let handle = app.clone();
        run_on_main_thread(&handle, move || {
            let panel = app_clone
                .get_webview_panel(WINDOW_LABEL)
                .map_err(|error| Error::OrbWindow(format!("{error:?}")))?;
            panel.show();
            Ok::<(), Error>(())
        })??;
    }

    #[cfg(not(target_os = "macos"))]
    {
        window.show().map_err(|error| {
            tracing::error!(%error, label = WINDOW_LABEL, "failed to show dictation orb window");
            Error::OrbWindow(error.to_string())
        })?;
    }

    let _ = window.set_always_on_top(true);

    // The caption window rides along hidden; its webview shows it only while
    // live text is on screen. A caption failure never blocks the orb.
    if let Err(error) = ensure_caption_window(app, &window) {
        tracing::warn!(
            %error,
            label = CAPTION_WINDOW_LABEL,
            "failed to create the dictation caption window; dictation \
             continues without the live caption"
        );
    }

    Ok(())
}

pub fn hide() -> Result<(), Error> {
    let app = app_handle()?;
    if let Some(caption) = app.get_webview_window(CAPTION_WINDOW_LABEL) {
        let _ = caption.hide();
    }

    #[cfg(target_os = "macos")]
    {
        // BUG 1 fix: hide through the NSPanel handle for parity with `show()`.
        use tauri_nspanel::ManagerExt;

        let app_clone = app.clone();
        let handle = app.clone();
        let _ = run_on_main_thread(&handle, move || {
            if let Ok(panel) = app_clone.get_webview_panel(WINDOW_LABEL) {
                panel.hide();
            }
            Ok::<(), Error>(())
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window
                .hide()
                .map_err(|error| Error::OrbWindow(error.to_string()))?;
        }
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
            Ok(_) => {
                // BUG 2 fix: the reuse path also re-clamps, so a monitor
                // change that stranded the orb off-screen self-heals without an
                // app restart (previously only the fresh-build path clamped).
                restore_position(app, &window);
                return Ok(window);
            }
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

    restore_position(app, &window);
    watch_moves(&window);

    Ok(window)
}

/// Run `f` on the AppKit main thread and return its value. Mirrors
/// `plugins/windows/src/ext::run_on_main_thread` (the dictation plugin has no
/// equivalent): the macOS panel build/show/hide must happen on the main thread.
/// macOS-only.
#[cfg(target_os = "macos")]
fn run_on_main_thread<R: Send + 'static>(
    app: &tauri::AppHandle<tauri::Wry>,
    f: impl FnOnce() -> R + Send + 'static,
) -> Result<R, Error> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|error| Error::OrbWindow(format!("failed to schedule on main thread: {error}")))?;

    rx.recv()
        .map_err(|_| Error::OrbWindow("main thread panel callback dropped".to_string()))
}

#[cfg(target_os = "macos")]
fn build_window(
    app: &tauri::AppHandle<tauri::Wry>,
    transparent: bool,
) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
    // BUG 1 fix: build the orb as a real NSPanel (`.nonactivatingPanel` style
    // mask) so clicking it never raises/activates the app. Mirrors
    // `plugins/windows/src/window/composer.rs::create`. The collection-behavior
    // bits `apply_macos_orb_traits` used to OR in by hand are set here at panel
    // construction: `full_screen_auxiliary` + `can_join_all_spaces` +
    // `stationary`, so the orb survives full-screen apps and Space switches.
    use tauri::{LogicalPosition, LogicalSize, Position, Size, WebviewUrl};
    use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

    let url = if transparent {
        WINDOW_URL
    } else {
        SOLID_WINDOW_URL
    };

    let app_clone = app.clone();
    let title = app
        .config()
        .product_name
        .clone()
        .unwrap_or_else(|| "Notare".to_string());

    run_on_main_thread(app, move || {
        PanelBuilder::<_, OrbPanel>::new(&app_clone, WINDOW_LABEL)
            .url(WebviewUrl::App(url.into()))
            .title(title)
            .position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .size(Size::Logical(LogicalSize::new(ORB_SIZE, ORB_SIZE)))
            .level(PanelLevel::Floating)
            .has_shadow(false)
            .collection_behavior(
                CollectionBehavior::new()
                    .full_screen_auxiliary()
                    .can_join_all_spaces()
                    .stationary(),
            )
            .hides_on_deactivate(false)
            .works_when_modal(true)
            .no_activate(true)
            .with_window(move |window| {
                let window = window
                    .visible(false)
                    .decorations(false)
                    .transparent(transparent)
                    .focused(false)
                    .focusable(false)
                    .accept_first_mouse(true)
                    .skip_taskbar(true)
                    .resizable(false)
                    .maximizable(false)
                    .minimizable(false);
                if !transparent {
                    window.background_color(SOLID_BACKGROUND)
                } else {
                    window
                }
            })
            .style_mask(StyleMask::empty().nonactivating_panel())
            .build()
            .map_err(|error| {
                tracing::error!(
                    %error,
                    label = WINDOW_LABEL,
                    url,
                    transparent,
                    "failed to build dictation orb panel"
                );
                Error::OrbWindow(format!(
                    "failed to create dictation orb panel `{WINDOW_LABEL}` (url `{url}`): {error}"
                ))
            })?;

        Ok::<(), Error>(())
    })??;

    // PanelBuilder registers the underlying webview window under WINDOW_LABEL
    // (same as composer.rs `ensure()`), so retrieve it for position/caption
    // handling. Position is set later by `restore_position`, not at build time.
    let window = app.get_webview_window(WINDOW_LABEL).ok_or_else(|| {
        Error::OrbWindow("dictation orb panel did not register a webview window".to_string())
    })?;

    // Materialization probe (same rationale as the non-macOS path): a panel
    // whose OS-side creation failed stays a ghost. `build()` ran on the main
    // thread above, so this round-trips through the event loop and surfaces a
    // failed creation as an error instead of an invisible panel.
    if let Err(error) = window.is_visible() {
        tracing::error!(
            %error,
            label = WINDOW_LABEL,
            url,
            transparent,
            "dictation orb panel did not materialize after build; check the \
             log for `tauri_runtime_wry` window/webview creation errors"
        );
        let _ = window.destroy();
        return Err(Error::OrbWindow(format!(
            "dictation orb panel `{WINDOW_LABEL}` was not created by the OS event loop: {error}"
        )));
    }

    tracing::info!(
        label = WINDOW_LABEL,
        url,
        transparent,
        "created dictation orb panel"
    );

    Ok(window)
}

#[cfg(not(target_os = "macos"))]
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
        // A non-activating (`focusable(false)`) window on macOS never becomes
        // key, so AppKit swallows the first left-click instead of delivering it
        // to the WKWebView - the orb button's onClick never fires and clicking
        // the orb does nothing (the global hotkey still works). `accept_first_mouse`
        // (default false) makes clicks pass through to the webview without
        // stealing key focus. No-op on Windows/Linux, where the click already
        // reaches the webview.
        .accept_first_mouse(true)
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

/// Actual logical size of `window`, falling back to the creation size. The
/// orb webview resizes the window per orb variant (1.5x for particles), so
/// positioning/clamping math must never assume the 56px creation constant.
fn window_logical_size(window: &tauri::WebviewWindow<tauri::Wry>) -> (f64, f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .outer_size()
        .map(|size| {
            let logical = size.to_logical::<f64>(scale);
            (logical.width, logical.height)
        })
        .unwrap_or((ORB_SIZE, ORB_SIZE))
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

    let (orb_width, orb_height) = window_logical_size(window);
    let scale = monitor.scale_factor();
    let position = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let x = position.x + ((size.width - orb_width) / 2.0);
    let y = position.y + size.height - orb_height - BOTTOM_MARGIN;
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

/// Place a newly created orb window at the persisted position (clamped to a
/// visible monitor), falling back to the default bottom-center spot when
/// nothing usable was saved (first run, or the saved monitor is gone).
fn restore_position(app: &tauri::AppHandle<tauri::Wry>, window: &tauri::WebviewWindow<tauri::Wry>) {
    if let Some(saved) = load_saved_position(app)
        && let Some(clamped) = clamp_to_visible_monitor(window, saved)
    {
        let _ = window.set_position(tauri::LogicalPosition::new(clamped.x, clamped.y));
        return;
    }

    position_bottom_center(app, window);
}

fn load_saved_position(app: &tauri::AppHandle<tauri::Wry>) -> Option<OrbPosition> {
    let store = app.store2().scoped_store::<String>(STORE_SCOPE).ok()?;
    store
        .get::<OrbPosition>(STORE_KEY_ORB_POSITION.to_string())
        .ok()
        .flatten()
}

/// Clamp `saved` so the orb lands fully inside the monitor its center falls
/// on, using the window's ACTUAL logical size (the webview may have resized
/// it per orb variant - 84px for particles, not the 56px creation constant).
/// Returns `None` when the center is on no current monitor (unplugged
/// screen, changed layout) - the caller then uses the default position.
fn clamp_to_visible_monitor(
    window: &tauri::WebviewWindow<tauri::Wry>,
    saved: OrbPosition,
) -> Option<OrbPosition> {
    let monitors = window.available_monitors().ok()?;
    let (orb_width, orb_height) = window_logical_size(window);

    let center_x = saved.x + orb_width / 2.0;
    let center_y = saved.y + orb_height / 2.0;

    for monitor in monitors {
        let scale = monitor.scale_factor();
        let position = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);

        let on_monitor = center_x >= position.x
            && center_x <= position.x + size.width
            && center_y >= position.y
            && center_y <= position.y + size.height;
        if !on_monitor {
            continue;
        }

        return Some(OrbPosition {
            x: saved
                .x
                .clamp(position.x, position.x + size.width - orb_width),
            y: saved
                .y
                .clamp(position.y, position.y + size.height - orb_height),
        });
    }

    None
}

/// Persist the window position (debounced) whenever the OS moves it - which
/// includes the user dragging the orb via `startDragging()` - and keep the
/// caption window glued above the orb on every move/resize (the webview
/// resizes the orb window per orb variant).
fn watch_moves(window: &tauri::WebviewWindow<tauri::Wry>) {
    let app = window.app_handle().clone();
    let win = window.clone();

    window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::Moved(position) => {
                let scale = win.scale_factor().unwrap_or(1.0);
                let logical = position.to_logical::<f64>(scale);
                schedule_position_save(
                    &app,
                    OrbPosition {
                        x: logical.x,
                        y: logical.y,
                    },
                );
                sync_caption_position(&app, &win);
            }
            tauri::WindowEvent::Resized(_) => {
                sync_caption_position(&app, &win);
            }
            _ => {}
        };
    });
}

/// Create the (hidden) live-caption window next to the orb if it does not
/// exist yet: transparent with the same solid fallback as the orb,
/// non-focusable AND fully click-through (`set_ignore_cursor_events`), so no
/// screen area around the orb ever swallows a click. Visibility is driven by
/// its webview (`caption.tsx`): shown while live words are on screen, hidden
/// again after the fade.
fn ensure_caption_window(
    app: &tauri::AppHandle<tauri::Wry>,
    orb: &tauri::WebviewWindow<tauri::Wry>,
) -> Result<(), Error> {
    let caption = match app.get_webview_window(CAPTION_WINDOW_LABEL) {
        Some(existing) => match existing.is_visible() {
            Ok(_) => existing,
            Err(error) => {
                tracing::warn!(
                    %error,
                    label = CAPTION_WINDOW_LABEL,
                    "existing dictation caption window is not backed by an \
                     OS window; destroying the ghost and recreating"
                );
                destroy_and_wait_unregistered_label(app, &existing, CAPTION_WINDOW_LABEL);
                build_caption_window(app)?
            }
        },
        None => build_caption_window(app)?,
    };

    // Click-through is the whole point of the second window; without it the
    // caption area would intercept clicks meant for whatever is behind it.
    if let Err(error) = caption.set_ignore_cursor_events(true) {
        tracing::warn!(
            %error,
            label = CAPTION_WINDOW_LABEL,
            "failed to make the dictation caption window click-through; \
             hiding it rather than letting it swallow clicks"
        );
        let _ = caption.destroy();
        return Err(Error::OrbWindow(error.to_string()));
    }

    sync_caption_position(app, orb);
    Ok(())
}

fn build_caption_window(
    app: &tauri::AppHandle<tauri::Wry>,
) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
    match build_secondary_window(
        app,
        CAPTION_WINDOW_LABEL,
        CAPTION_WINDOW_URL,
        (CAPTION_WIDTH, CAPTION_HEIGHT),
        true,
    ) {
        Ok(window) => Ok(window),
        Err(error) => {
            tracing::warn!(
                %error,
                label = CAPTION_WINDOW_LABEL,
                "transparent dictation caption window failed to create; \
                 retrying with a solid background"
            );
            if let Some(ghost) = app.get_webview_window(CAPTION_WINDOW_LABEL) {
                destroy_and_wait_unregistered_label(app, &ghost, CAPTION_WINDOW_LABEL);
            }
            build_secondary_window(
                app,
                CAPTION_WINDOW_LABEL,
                CAPTION_SOLID_WINDOW_URL,
                (CAPTION_WIDTH, CAPTION_HEIGHT),
                false,
            )
        }
    }
}

/// Shared builder for the plugin's auxiliary windows (currently only the
/// caption): same chassis flags as the orb, created hidden.
fn build_secondary_window(
    app: &tauri::AppHandle<tauri::Wry>,
    label: &str,
    url: &str,
    (width, height): (f64, f64),
    transparent: bool,
) -> Result<tauri::WebviewWindow<tauri::Wry>, Error> {
    use tauri::{WebviewUrl, WebviewWindow};

    let mut builder = WebviewWindow::builder(app, label, WebviewUrl::App(url.into()))
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
        .focusable(false)
        .visible(false)
        .inner_size(width, height)
        .disable_drag_drop_handler();
    if !transparent {
        builder = builder.background_color(SOLID_BACKGROUND);
    }

    let window = builder.build().map_err(|error| {
        tracing::error!(%error, label, url, transparent, "failed to build dictation window");
        Error::OrbWindow(format!(
            "failed to create dictation window `{label}` (url `{url}`): {error}"
        ))
    })?;

    // Same materialization probe as the orb window: `build()` off the main
    // thread cannot report OS-side creation failures directly.
    if let Err(error) = window.is_visible() {
        tracing::error!(
            %error,
            label,
            url,
            transparent,
            "dictation window did not materialize after build"
        );
        let _ = window.destroy();
        return Err(Error::OrbWindow(format!(
            "dictation window `{label}` was not created by the OS event loop: {error}"
        )));
    }

    tracing::info!(label, url, transparent, "created dictation window");

    Ok(window)
}

/// Keep the caption window centered above the orb (or below it when the orb
/// sits at the very top of its monitor), clamped to the monitor edges.
fn sync_caption_position(
    app: &tauri::AppHandle<tauri::Wry>,
    orb: &tauri::WebviewWindow<tauri::Wry>,
) {
    let Some(caption) = app.get_webview_window(CAPTION_WINDOW_LABEL) else {
        return;
    };

    let scale = orb.scale_factor().unwrap_or(1.0);
    let Ok(orb_position) = orb.outer_position() else {
        return;
    };
    let orb_position = orb_position.to_logical::<f64>(scale);
    let (orb_width, orb_height) = window_logical_size(orb);

    let mut x = orb_position.x + (orb_width - CAPTION_WIDTH) / 2.0;
    let mut y = orb_position.y - CAPTION_HEIGHT - CAPTION_GAP;

    if let Ok(Some(monitor)) = orb.current_monitor() {
        let monitor_scale = monitor.scale_factor();
        let monitor_position = monitor.position().to_logical::<f64>(monitor_scale);
        let monitor_size = monitor.size().to_logical::<f64>(monitor_scale);

        x = x.clamp(
            monitor_position.x,
            (monitor_position.x + monitor_size.width - CAPTION_WIDTH).max(monitor_position.x),
        );
        if y < monitor_position.y {
            // No room above (orb at the top edge): drop below the orb.
            y = orb_position.y + orb_height + CAPTION_GAP;
        }
    }

    let _ = caption.set_position(tauri::LogicalPosition::new(x, y));
}

/// Label-parameterized twin of `destroy_and_wait_unregistered` (which is
/// hard-wired to the orb label).
fn destroy_and_wait_unregistered_label(
    app: &tauri::AppHandle<tauri::Wry>,
    window: &tauri::WebviewWindow<tauri::Wry>,
    label: &str,
) {
    let _ = window.destroy();

    for _ in 0..25 {
        if app.get_webview_window(label).is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    tracing::warn!(
        label,
        "dictation window still registered after destroy; a rebuild with \
         the same label may fail with a duplicate-label error"
    );
}

fn schedule_position_save(app: &tauri::AppHandle<tauri::Wry>, position: OrbPosition) {
    {
        let mut pending = PENDING_POSITION.lock().unwrap_or_else(|e| e.into_inner());
        *pending = Some(position);
    }

    if SAVE_TASK_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POSITION_SAVE_DEBOUNCE).await;
            let position = PENDING_POSITION
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let Some(position) = position else {
                break;
            };
            persist_position(&app, position);
        }

        SAVE_TASK_RUNNING.store(false, Ordering::SeqCst);
        // A move that landed between the final take() and the flag clear
        // would otherwise be lost until the next drag.
        let straggler = PENDING_POSITION
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(position) = straggler {
            persist_position(&app, position);
        }
    });
}

fn persist_position(app: &tauri::AppHandle<tauri::Wry>, position: OrbPosition) {
    let store = match app.store2().scoped_store::<String>(STORE_SCOPE) {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!(%error, "failed to open the store for the dictation orb position");
            return;
        }
    };

    if let Err(error) = store
        .set(STORE_KEY_ORB_POSITION.to_string(), position)
        .and_then(|()| store.save())
    {
        tracing::warn!(%error, "failed to persist the dictation orb position");
    }
}
