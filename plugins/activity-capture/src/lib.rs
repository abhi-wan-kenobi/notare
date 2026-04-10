use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tauri::Manager;

mod analysis;
mod commands;
mod error;
mod events;
mod ext;
pub mod runtime;

pub use error::{Error, Result};
pub use events::*;
pub use ext::*;

const PLUGIN_NAME: &str = "activity-capture";

pub type ManagedState<R> = Arc<runtime::ActivityCaptureRuntime<R>>;

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::capabilities::<tauri::Wry>,
            commands::snapshot::<tauri::Wry>,
            commands::latest_screenshot_analysis::<tauri::Wry>,
            commands::list_analyses_in_range::<tauri::Wry>,
            commands::status::<tauri::Wry>,
            commands::start::<tauri::Wry>,
            commands::stop::<tauri::Wry>,
            commands::is_running::<tauri::Wry>,
            commands::configure::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![ActivityCapturePluginEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let db_path = app
                .path()
                .app_data_dir()
                .expect("app_data_dir must be available")
                .join("activity.db");

            let pool = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create db init runtime");
                rt.block_on(open_or_recreate(&db_path))
            })
            .join()
            .expect("db init thread panicked");

            app.manage(Arc::new(runtime::ActivityCaptureRuntime::new(
                app.app_handle().clone(),
                Arc::new(pool),
            )));
            Ok(())
        })
        .build()
}

async fn connect(path: &Path) -> std::result::Result<SqlitePool, sqlx::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("foreign_keys", "ON");
    SqlitePoolOptions::new().connect_with(options).await
}

async fn open_or_recreate(path: &PathBuf) -> SqlitePool {
    match try_open(path).await {
        Ok(pool) => pool,
        Err(error) => {
            tracing::warn!(
                %error,
                path = %path.display(),
                "activity db migration failed, wiping and recreating"
            );
            wipe_db_files(path);
            try_open(path)
                .await
                .expect("failed to create fresh activity db")
        }
    }
}

async fn try_open(path: &Path) -> std::result::Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = connect(path).await?;
    hypr_db_activity::migrate(&pool).await?;
    Ok(pool)
}

fn wipe_db_files(path: &Path) {
    for suffix in &["", "-wal", "-shm", "-journal"] {
        let file = PathBuf::from(format!("{}{suffix}", path.display()));
        if file.exists() {
            if let Err(error) = std::fs::remove_file(&file) {
                tracing::warn!(%error, path = %file.display(), "failed to remove db file");
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn test_plugin_init() {
        let _app = create_app(tauri::test::mock_builder());
    }
}
