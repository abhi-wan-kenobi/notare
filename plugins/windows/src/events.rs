use tauri::Manager;

use crate::HyprWindow;

pub fn on_window_event<R: tauri::Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if let Ok(w) = window.label().parse::<HyprWindow>() {
                if w == HyprWindow::Main {
                    if let Ok(_) = window.hide() {
                        api.prevent_close();
                    }
                }
            }
        }
        tauri::WindowEvent::Destroyed { .. } => {
            let app = window.app_handle();
            let state = app.state::<crate::ManagedState>();

            {
                let mut guard = state.lock().unwrap();

                if let Ok(w) = window.label().parse::<HyprWindow>() {
                    guard.windows.remove(&w);
                }
            }
        }
        _ => {}
    }
}

#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
pub struct NavigateMain {
    pub path: String,
}
