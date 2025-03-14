#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
pub struct NavigateMain {
    pub path: String,
}
