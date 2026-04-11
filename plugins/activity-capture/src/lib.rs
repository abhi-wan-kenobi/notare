use std::sync::Arc;

use hypr_db_core2::{Db3, DbOpenOptions, DbStorage, MigrationFailurePolicy};
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
            commands::get_daily_summary_snapshot::<tauri::Wry>,
            commands::save_daily_summary::<tauri::Wry>,
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

            let db = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create db init runtime");
                rt.block_on(Db3::open_with_migrate(
                    DbOpenOptions {
                        storage: DbStorage::Local(&db_path),
                        cloudsync: false,
                        journal_mode_wal: true,
                        foreign_keys: true,
                        max_connections: None,
                        migration_failure_policy: MigrationFailurePolicy::Recreate,
                    },
                    |pool| Box::pin(hypr_db_activity::migrate(pool)),
                ))
            })
            .join()
            .expect("db init thread panicked")
            .expect("failed to initialize activity database");

            app.manage(Arc::new(runtime::ActivityCaptureRuntime::new(
                app.app_handle().clone(),
                Arc::new(db.pool().clone()),
            )));
            Ok(())
        })
        .build()
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
