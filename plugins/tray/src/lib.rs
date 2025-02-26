mod ext;
pub use ext::*;

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("tray")
        .invoke_handler(tauri::generate_handler![])
        .build()
}
