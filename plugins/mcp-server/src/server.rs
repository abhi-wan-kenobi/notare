use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use tauri_plugin_tantivy::TantivyPluginExt;

use crate::types::*;

#[derive(Clone)]
pub struct NotareMcpServer {
    pool: sqlx::SqlitePool,
    app_handle: Option<tauri::AppHandle>,
}

impl NotareMcpServer {
    pub fn new(pool: sqlx::SqlitePool, app_handle: Option<tauri::AppHandle>) -> Self {
        Self { pool, app_handle }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
}

fn structured<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(internal_err)
}

fn internal_err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(idx) {
        idx = idx.saturating_sub(1);
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub fn extract_snippet(text: &str, query: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_text.find(&lower_query) {
        let raw_start = pos.saturating_sub(60);
        let start = floor_char_boundary(text, raw_start.min(text.len()));

        let raw_end = pos.saturating_add(query.len()).saturating_add(60);
        let end = ceil_char_boundary(text, raw_end.min(text.len()));
        let start = start.min(end);

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < text.len() { "..." } else { "" };

        let snippet = &text[start..end];
        format!("{prefix}{snippet}{suffix}")
    } else {
        let raw_end = 120.min(text.len());
        let end = ceil_char_boundary(text, raw_end);
        let suffix = if end < text.len() { "..." } else { "" };
        format!("{}{suffix}", &text[..end])
    }
}

#[tool_router]
impl NotareMcpServer {
    #[tool(
        description = "List recent Notare meetings with pagination metadata. Use query to narrow by title or meeting id, then pass next_offset as offset to continue.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn list_meetings(
        &self,
        Parameters(params): Parameters<ListMeetingsParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = hypr_agent_access::ListMeetingsInput {
            query: params.query,
            series_id: None,
            limit: params.limit,
            offset: params.offset,
        };

        let page = hypr_agent_access::list_meetings(&self.pool, input)
            .await
            .map_err(internal_err)?;

        structured(&page)
    }

    #[tool(
        description = "Get one Notare meeting with its canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for transcript words.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn get_meeting(
        &self,
        Parameters(params): Parameters<GetMeetingParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = hypr_agent_access::GetMeetingInput {
            meeting_id: params.id,
        };

        let meeting = hypr_agent_access::get_meeting(&self.pool, input)
            .await
            .map_err(internal_err)?;

        structured(&meeting)
    }

    #[tool(
        description = "Get a bounded page of transcript words and readable text for a Notare meeting. Pass pagination.next_offset as offset to continue.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn get_meeting_transcript(
        &self,
        Parameters(params): Parameters<GetMeetingTranscriptParams>,
    ) -> Result<CallToolResult, McpError> {
        let input = hypr_agent_access::GetMeetingTranscriptInput {
            meeting_id: params.id,
            offset: params.offset,
            limit: params.limit,
        };

        let page = hypr_agent_access::get_meeting_transcript(&self.pool, input)
            .await
            .map_err(internal_err)?;

        structured(&page)
    }

    #[tool(
        description = "Search meetings using full-text indexing (Tantivy). Returns matching meetings with scores and snippets.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn search_meetings(
        &self,
        Parameters(params): Parameters<SearchMeetingsParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(app) = &self.app_handle {
            let request = tauri_plugin_tantivy::SearchRequest {
                query: params.query.clone(),
                collection: None,
                filters: tauri_plugin_tantivy::SearchFilters::default(),
                options: tauri_plugin_tantivy::SearchOptions::default(),
                limit: params.limit.unwrap_or(20),
            };

            match app.tantivy().search(request).await {
                Ok(result) => return structured(&result),
                Err(err) => {
                    tracing::warn!(
                        "Tantivy search returned error: {err}, falling back to DB title search"
                    );
                }
            }
        }

        // Fallback or test mode: search meetings in SQLite sessions table
        let query_pattern = format!("%{}%", params.query);
        let limit = params.limit.unwrap_or(20) as i64;
        let offset = params.offset.unwrap_or(0) as i64;

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, title FROM sessions WHERE deleted_at IS NULL AND title LIKE ? ORDER BY updated_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&query_pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(internal_err)?;

        let results: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, title)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                })
            })
            .collect();

        structured(&results)
    }

    #[tool(
        description = "Lexical content search across meeting notes and transcripts. Searches notes and transcripts for exact words or phrases. Note: plugins/embedding-search does not expose a hybrid search Rust API, so this searches lexically.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn search_meeting_content(
        &self,
        Parameters(params): Parameters<SearchMeetingContentParams>,
    ) -> Result<CallToolResult, McpError> {
        let pattern = format!("%{}%", params.query);
        let limit = params.limit.unwrap_or(20);
        let mut matches = Vec::new();

        // 1. Search session_documents
        let doc_rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT session_id, kind, title, body FROM session_documents WHERE deleted_at IS NULL AND (title LIKE ?1 OR body LIKE ?1) LIMIT 100",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(internal_err)?;

        for (session_id, kind, title, body) in doc_rows {
            if let Some(filter_ids) = &params.meeting_ids
                && !filter_ids.contains(&session_id)
            {
                continue;
            }

            let snippet = extract_snippet(&body, &params.query);
            matches.push(ContentMatch {
                meeting_id: session_id,
                source: format!("document:{kind}:{title}"),
                snippet,
                score: 1.0,
            });
            if matches.len() >= limit {
                break;
            }
        }

        // 2. Search transcripts if limit not yet reached
        if matches.len() < limit {
            let transcript_rows: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT session_id, memo, words_json FROM transcripts WHERE deleted_at IS NULL AND (memo LIKE ?1 OR words_json LIKE ?1) LIMIT 100",
            )
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(internal_err)?;

            for (session_id, memo, words_json) in transcript_rows {
                if let Some(filter_ids) = &params.meeting_ids
                    && !filter_ids.contains(&session_id)
                {
                    continue;
                }

                let text = if memo.is_empty() { words_json } else { memo };
                let snippet = extract_snippet(&text, &params.query);
                matches.push(ContentMatch {
                    meeting_id: session_id,
                    source: "transcript".to_string(),
                    snippet,
                    score: 0.8,
                });

                if matches.len() >= limit {
                    break;
                }
            }
        }

        let output = SearchMeetingContentResult {
            query: params.query,
            mode: "lexical (plugins/embedding-search does not expose a hybrid API)".to_string(),
            results: matches,
        };

        structured(&output)
    }

    #[tool(
        description = "List all available AI recipes and prompts in Notare.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn list_recipes(
        &self,
        Parameters(_): Parameters<ListRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        let recipes = tauri_plugin_db::list_recipes_db(&self.pool)
            .await
            .map_err(internal_err)?;

        structured(&recipes)
    }

    #[tool(
        description = "Get the daily note for a given date (YYYY-MM-DD format).",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn get_daily_note(
        &self,
        Parameters(params): Parameters<GetDailyNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        let note: Option<DailyNoteRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner_user_id, note_date, body_format, body, created_at, updated_at FROM daily_notes WHERE note_date = ? AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(&params.date)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal_err)?;

        match note {
            Some(row) => structured(&row),
            None => Ok(CallToolResult::structured(serde_json::json!({
                "message": format!("No daily note found for date {}", params.date),
                "daily_note": null
            }))),
        }
    }
}

#[tool_handler]
impl ServerHandler for NotareMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::LATEST)
        .with_server_info(Implementation::new(
            "notare-mcp-server",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(
            "Read-only access to Notare meetings, transcripts, search, recipes, and daily notes.",
        )
    }
}
