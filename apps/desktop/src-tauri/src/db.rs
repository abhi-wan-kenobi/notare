use std::sync::Arc;

use hypr_db_core2::Db3;

pub async fn open_desktop_db(identifier: &str) -> Arc<Db3> {
    let base = dirs::data_dir().expect("data_dir must be available");

    let db_path = match identifier {
        "com.hyprnote.dev" => None,
        "com.hyprnote.stable" | "com.hyprnote.nightly" => Some(base.join("hyprnote")),
        _ => Some(base.join(identifier)),
    }
    .map(|dir| {
        std::fs::create_dir_all(&dir).expect("failed to create app data dir");
        dir.join("app.db")
    });

    Arc::new(
        tauri_plugin_db::open_app_db(db_path.as_deref())
            .await
            .expect("failed to open app database"),
    )
}
