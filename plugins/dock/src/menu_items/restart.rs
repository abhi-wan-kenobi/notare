use super::DockMenuItem;

pub struct DockRestart;

impl DockMenuItem for DockRestart {
    fn title(app: &tauri::AppHandle<tauri::Wry>) -> String {
        format!("Restart {}", app.package_info().name)
    }

    fn handle(app: &tauri::AppHandle<tauri::Wry>) {
        // DRAFT / data-loss follow-up (crash-resilience hardening pass): a
        // user-initiated restart here bypasses `RunEvent::ExitRequested`, so the
        // frontend `flushAndExit` finalize does NOT run — an in-flight recording
        // would be dropped. The proper fix is a single shared "finalize active
        // capture, then restart" choke point all restart()/exit paths route
        // through (mirrors `finalize_capture_before_restart` in the app's
        // supervisor). Not wired here yet to avoid a cross-crate dep; tracked in
        // the session-durability invariant work.
        app.restart();
    }
}
