use tauri_plugin_store2::ScopedStoreKey;

#[derive(serde::Deserialize, specta::Type, PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    CustomEnabled,
    CustomApiBase,
    CustomApiKey,
    CustomModel,
    AdminApiBase,
    AdminApiKey,
    OpenaiApiKey,
    OpenrouterApiKey,
    GeminiApiKey,
}

impl ScopedStoreKey for StoreKey {}
