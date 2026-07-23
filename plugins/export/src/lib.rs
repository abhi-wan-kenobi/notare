mod action_items;
mod commands;
mod ext;
pub use action_items::ExportActionItemsFormat;
pub use ext::*;
pub use hypr_export_core::{
    Error, ExportInput, ExportMetadata, Result, Transcript, TranscriptItem,
};

use std::sync::Arc;

use tauri::Manager;

const PLUGIN_NAME: &str = "export";

/// Managed state: the shared app DB, used by `export_action_items` to read the
/// `action_items` rows. (The PDF `export` command takes its data as input and
/// does not touch this.)
pub type ManagedState = Arc<hypr_db_core::Db>;

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::export::<tauri::Wry>,
            commands::export_action_items,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(db: Arc<hypr_db_core::Db>) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            app.manage(db);
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

    fn create_app<R: tauri::Runtime>(
        builder: tauri::Builder<R>,
        db: Arc<hypr_db_core::Db>,
    ) -> tauri::App<R> {
        let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
        ctx.config_mut().identifier = "com.hyprnote.dev".to_string();
        ctx.config_mut().version = Some("0.0.1".to_string());

        builder.plugin(init(db)).build(ctx).unwrap()
    }

    #[ignore]
    #[tokio::test]
    async fn test_export_pdf() {
        let db = Arc::new(hypr_db_core::Db::connect_memory_plain().await.unwrap());
        let app = create_app(tauri::test::mock_builder(), db);

        app.export()
            .export_pdf(
                "test.pdf",
                ExportInput {
                    enhanced_md: "# Test Document\n\nThis is a test.".to_string(),
                    memo_md: None,
                    transcript: Some(Transcript {
                        items: vec![TranscriptItem {
                            speaker: Some("Speaker 1".to_string()),
                            text: "Hello, world!".to_string(),
                        }],
                    }),
                    metadata: None,
                },
            )
            .unwrap();
    }
}
