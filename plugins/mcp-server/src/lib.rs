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
            // Never panic in setup: a missing dependency or unreadable store must
            // not take the whole app down (0.6.2 did exactly that).
            let Some(pool) = app
                .try_state::<tauri_plugin_db::ManagedState>()
                .map(|s| s.pool().clone())
            else {
                tracing::error!("tauri-plugin-db is not initialized; MCP server disabled");
                return Ok(());
            };

            let manager = McpServerManager::new(pool, app.clone());

            // Auto-start only reads the plain store (no keyring) so a disabled
            // server never costs a keychain prompt at launch.
            match manager.autostart_config() {
                Ok((true, port)) => {
                    let mgr = manager.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = mgr.restart(port).await {
                            tracing::error!("Failed to auto-start MCP server: {e}");
                        }
                    });
                }
                Ok((false, _)) => {}
                Err(e) => tracing::warn!("MCP server settings unreadable at startup: {e}"),
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
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

        let server_handle = tokio::spawn(http::run_server(
            server,
            port,
            token.to_string(),
            shutdown_rx,
            Some(ready_tx),
        ));

        ready_rx.await.unwrap().unwrap();

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
        let result =
            http::run_server(server, occupied_port, "test".to_string(), shutdown_rx, None).await;

        drop(listener);

        match result {
            Err(Error::PortInUse(p)) => assert_eq!(p, occupied_port),
            other => panic!("expected PortInUse error, got {other:?}"),
        }
    }

    #[test]
    fn test_extract_snippet_char_boundaries_devanagari_and_emoji() {
        // Hindi transcript with Devanagari and emoji
        let text = "यहाँ पर नमस्ते दुनिया 😀 है और हम सब मिलकर बात कर रहे हैं। इस मीटिंग में बहुत सारे महत्वपूर्ण निर्णय लिए गए।";

        // 1. Search for word in middle
        let snippet = server::extract_snippet(text, "महत्वपूर्ण");
        assert!(snippet.contains("महत्वपूर्ण"));

        // 2. Search for emoji
        let snippet_emoji = server::extract_snippet(text, "😀");
        assert!(snippet_emoji.contains("😀"));

        // 3. Specifically test byte offset that would land in the middle of a 4-byte emoji
        // 😀 is 4 bytes: [240, 159, 152, 128]
        // Place 😀 at byte 57..61, so byte 60 is inside 😀
        let mut crafted = "a".repeat(57);
        crafted.push('😀');
        crafted.push_str(&"b".repeat(70));
        // Query "bb" starts at byte 61 + 59 = 120. pos - 60 = 60, which falls inside '😀' (bytes 57..61)!
        // In old code, &crafted[60..] panicked with: "byte index 60 is not a char boundary"
        let query = "bb";
        let snippet_crafted = server::extract_snippet(&crafted, query);
        assert!(snippet_crafted.contains("bb"));

        // 4. Missing query with multibyte text (fallback slicing)
        let snippet_fallback = server::extract_snippet(text, "nonexistent");
        assert!(!snippet_fallback.is_empty());
    }

    #[test]
    fn test_port_validation() {
        use crate::ext::validate_port;

        // Port 0 is always rejected
        assert!(matches!(validate_port(0, None), Err(Error::InvalidPort(0))));
        assert!(matches!(
            validate_port(0, Some(0)),
            Err(Error::InvalidPort(0))
        ));

        // Privileged port < 1024 is rejected unless already running
        assert!(matches!(
            validate_port(80, None),
            Err(Error::InvalidPort(80))
        ));
        assert!(matches!(
            validate_port(443, Some(80)),
            Err(Error::InvalidPort(443))
        ));
        assert!(validate_port(80, Some(80)).is_ok());

        // Non-privileged port >= 1024 is accepted
        assert!(validate_port(8787, None).is_ok());
        assert!(validate_port(1024, None).is_ok());
    }

    #[tokio::test]
    async fn test_empty_bearer_token_rejected() {
        let (_dir, db) = setup_test_db().await;
        let server = NotareMcpServer::new(db.pool().clone(), None);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let result = http::run_server(server, 8787, "   ".to_string(), shutdown_rx, None).await;
        assert!(matches!(result, Err(Error::Internal(_))));
    }

    #[tokio::test]
    async fn test_search_meeting_content_db_error_surfaced() {
        let (_dir, db) = setup_test_db().await;
        let pool = db.pool().clone();
        let server = NotareMcpServer::new(pool.clone(), None);

        // Close the pool to induce a DB error
        pool.close().await;

        let res = server
            .search_meeting_content(Parameters(SearchMeetingContentParams {
                query: "test".to_string(),
                meeting_ids: None,
                limit: Some(10),
            }))
            .await;

        // Must return Err(McpError) rather than Ok with empty matches!
        assert!(
            res.is_err(),
            "expected DB error to be surfaced as MCP error"
        );
    }
}
