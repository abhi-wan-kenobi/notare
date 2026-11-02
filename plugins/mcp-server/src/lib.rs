pub mod commands;
pub mod error;
pub mod ext;
pub mod http;
pub mod server;
pub mod stdio_bridge;
pub mod types;

use tauri::Manager;

pub use error::{Error, Result};
pub use ext::{McpServerManager, McpServerState};
pub use server::NotareMcpServer;
pub use types::*;

const PLUGIN_NAME: &str = "mcp-server";

pub fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::get_settings,
            commands::set_settings,
            commands::regenerate_token,
            commands::get_status,
        ])
        .typ::<types::McpServerSettings>()
        .typ::<types::McpServerSettingsInput>()
        .typ::<types::McpServerStatus>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            let pool = app
                .try_state::<tauri_plugin_db::ManagedState>()
                .map(|s| s.pool().clone())
                .expect("tauri-plugin-db must be initialized before tauri-plugin-mcp-server");

            let manager = McpServerManager::new(pool, app.clone());

            // If enabled in settings, auto-start
            if let Ok(settings) = manager.get_settings()
                && settings.enabled {
                    let port = settings.port;
                    let mgr_state = manager.state.clone();
                    let app_clone = app.clone();
                    let pool_clone = manager.pool.clone();
                    let token = settings.bearer_token;

                    tauri::async_runtime::spawn(async move {
                        let (tx, rx) = tokio::sync::watch::channel(false);
                        {
                            let mut guard = mgr_state.write().await;
                            guard.running = true;
                            guard.port = port;
                            guard.shutdown_tx = Some(tx);
                        }
                        let server = NotareMcpServer::new(pool_clone, Some(app_clone));
                        if let Err(e) = http::run_server(server, port, token, rx).await {
                            tracing::error!("Failed to auto-start MCP server: {e}");
                            let mut guard = mgr_state.write().await;
                            guard.running = false;
                            guard.last_error = Some(e.to_string());
                        }
                    });
                }

            app.manage(manager);
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::wrapper::Parameters;
    use std::time::Duration;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
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

    async fn setup_test_db() -> (tempfile::TempDir, hypr_db_core::Db) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn test_tool_handlers_against_sqlite() {
        let (_dir, db) = setup_test_db().await;
        let pool = db.pool().clone();

        // Seed test data
        sqlx::query(
            "INSERT INTO sessions (id, title, created_at, updated_at) VALUES ('m-1', 'Sprint Retrospective', '2026-11-02T10:00:00Z', '2026-11-02T11:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, title, body) VALUES ('doc-1', 'm-1', 'note', 'Summary', 'Discussed Q4 achievements and retrospective improvements.')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO session_participants (id, session_id, display_name, email) VALUES ('part-1', 'm-1', 'Bob Vance', 'bob@example.com')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO action_items (id, session_id, text, status) VALUES ('act-1', 'm-1', 'Review architecture PR', 'todo')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO transcripts (id, session_id, memo, words_json) VALUES ('tr-1', 'm-1', 'Audio transcript content goes here.', '[{\"text\":\"Audio\"},{\"text\":\"transcript\"},{\"text\":\"content\"}]')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO recipes (id, title, prompt, scope, builtin) VALUES ('rec-1', 'Action Items Extractor', 'Extract action items from note', 'meeting', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO daily_notes (id, note_date, body_format, body) VALUES ('dn-1', '2026-11-02', 'prosemirror_json', '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"text\":\"Daily note for sprint\"}]}')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let server = NotareMcpServer::new(pool, None);

        // 1. list_meetings
        let list_res = server
            .list_meetings(Parameters(ListMeetingsParams {
                query: None,
                limit: Some(10),
                offset: None,
            }))
            .await
            .unwrap();
        let list_json = serde_json::to_string(&list_res).unwrap();
        assert!(
            list_json.contains("Sprint Retrospective"),
            "expected meeting in list"
        );

        // 2. get_meeting
        let get_res = server
            .get_meeting(Parameters(GetMeetingParams {
                id: "m-1".to_string(),
            }))
            .await
            .unwrap();
        let get_json = serde_json::to_string(&get_res).unwrap();
        assert!(get_json.contains("Sprint Retrospective"));
        assert!(get_json.contains("Bob Vance"));
        assert!(get_json.contains("Review architecture PR"));

        // 3. get_meeting_transcript
        let tr_res = server
            .get_meeting_transcript(Parameters(GetMeetingTranscriptParams {
                id: "m-1".to_string(),
                offset: None,
                limit: None,
            }))
            .await
            .unwrap();
        let tr_json = serde_json::to_string(&tr_res).unwrap();
        assert!(tr_json.contains("Audio transcript content"));

        // 4. search_meetings
        let search_res = server
            .search_meetings(Parameters(SearchMeetingsParams {
                query: "Retrospective".to_string(),
                limit: Some(10),
                offset: None,
            }))
            .await
            .unwrap();
        let search_json = serde_json::to_string(&search_res).unwrap();
        assert!(search_json.contains("Sprint Retrospective"));

        // 5. search_meeting_content (lexical)
        let content_res = server
            .search_meeting_content(Parameters(SearchMeetingContentParams {
                query: "retrospective improvements".to_string(),
                meeting_ids: None,
                limit: Some(10),
            }))
            .await
            .unwrap();
        let content_json = serde_json::to_string(&content_res).unwrap();
        assert!(content_json.contains("retrospective improvements"));
        assert!(content_json.contains("lexical"));

        // 6. list_recipes
        let recipes_res = server
            .list_recipes(Parameters(ListRecipesParams {}))
            .await
            .unwrap();
        let rec_json = serde_json::to_string(&recipes_res).unwrap();
        assert!(rec_json.contains("Action Items Extractor"));

        // 7. get_daily_note
        let dn_res = server
            .get_daily_note(Parameters(GetDailyNoteParams {
                date: "2026-11-02".to_string(),
            }))
            .await
            .unwrap();
        let dn_json = serde_json::to_string(&dn_res).unwrap();
        assert!(dn_json.contains("Daily note for sprint"));

        // 7b. get_daily_note non-existent
        let dn_missing = server
            .get_daily_note(Parameters(GetDailyNoteParams {
                date: "2099-01-01".to_string(),
            }))
            .await
            .unwrap();
        let dn_missing_json = serde_json::to_string(&dn_missing).unwrap();
        assert!(dn_missing_json.contains("No daily note found"));
    }

    #[tokio::test]
    async fn test_auth_rejection_without_bearer() {
        let (_dir, db) = setup_test_db().await;
        let server = NotareMcpServer::new(db.pool().clone(), None);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let token = "valid-secret-token-test";
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let server_handle = tokio::spawn(http::run_server(
            server,
            port,
            token.to_string(),
            shutdown_rx,
        ));

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/");

        // Request without bearer token -> 401
        let no_auth = client.post(&url).body("{}").send().await.unwrap();
        assert_eq!(no_auth.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Request with wrong bearer token -> 401
        let wrong_auth = client
            .post(&url)
            .header("Authorization", "Bearer invalid-token")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_auth.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Request with valid bearer token -> NOT 401
        let valid_auth = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#)
            .send()
            .await
            .unwrap();
        assert_ne!(valid_auth.status(), reqwest::StatusCode::UNAUTHORIZED);

        let _ = shutdown_tx.send(true);
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_port_in_use_error_surfaces() {
        let (_dir, db) = setup_test_db().await;
        let server = NotareMcpServer::new(db.pool().clone(), None);

        // Bind port first
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let occupied_port = listener.local_addr().unwrap().port();

        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let result = http::run_server(server, occupied_port, "test".to_string(), shutdown_rx).await;

        drop(listener);

        match result {
            Err(Error::PortInUse(p)) => assert_eq!(p, occupied_port),
            other => panic!("expected PortInUse error, got {other:?}"),
        }
    }
}
