use std::collections::HashMap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use tauri_plugin_embedding_search::EmbeddingSearchPluginExt;
use tauri_plugin_tantivy::TantivyPluginExt;

use crate::rrf::{RRF_K, RankedItem, fuse_arms};
use crate::types::*;

/// KNN depth for the semantic arm's candidate pool before RRF fusion and
/// `limit` truncation — matches the dense-arm `k` used by chat pre-retrieval
/// (`apps/desktop/src/chat/transport/pre-retrieval.ts`).
const SEMANTIC_KNN_K: u32 = 60;

/// Above this many ids, `meeting_ids` falls back to one global semantic
/// query plus a post-hoc filter instead of one query per id, bounding the
/// number of embed calls a single request can trigger.
const MAX_MEETING_ID_FANOUT: usize = 20;

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

/// Build a `LIKE` pattern that treats `query` as a literal substring: escapes
/// `\`, `%`, and `_` so a query containing SQL wildcards (e.g. `"100%"` or
/// `"foo_bar"`) cannot silently widen the match set. Pair with `ESCAPE '\'`
/// in the SQL.
pub fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Resolve the semantic (embedding KNN) arm: ranked session ids for RRF,
/// their chunk hits grouped by session (closest-first, as `knn_join`
/// already returns them), and — when the arm could not run — a reason
/// string for the result's `mode` field. Never returns an `Err`: a missing
/// runtime, an undownloaded model, or a search failure all degrade to an
/// empty arm plus an explanatory reason, so the caller always has lexical
/// results to fall back to.
///
/// `unavailable_reason` is the reason to report when `runtime` is `None`
/// (the caller knows whether that's "no app context" or "plugin not
/// registered"; this function only knows what to do once it has a runtime).
pub(crate) async fn semantic_arm(
    runtime: Option<std::sync::Arc<tauri_plugin_embedding_search::EmbeddingSearchRuntime>>,
    unavailable_reason: Option<&str>,
    query: &str,
    meeting_ids: Option<&[String]>,
) -> (
    Vec<RankedItem>,
    HashMap<String, Vec<SemanticContentMatch>>,
    Option<String>,
) {
    let mut ranked = Vec::new();
    let mut by_session: HashMap<String, Vec<SemanticContentMatch>> = HashMap::new();

    let Some(runtime) = runtime else {
        let reason = unavailable_reason
            .unwrap_or("embedding-search plugin not registered")
            .to_string();
        return (ranked, by_session, Some(reason));
    };

    match runtime.index_status().await {
        Err(e) => {
            tracing::warn!("embedding-search index status failed: {e}");
            return (
                ranked,
                by_session,
                Some(format!("index status unavailable: {e}")),
            );
        }
        Ok(status) if !status.model_downloaded => {
            return (
                ranked,
                by_session,
                Some("semantic model not downloaded yet".to_string()),
            );
        }
        Ok(_) => {}
    }

    // A small `meeting_ids` filter is queried per-id so a chunk ranked
    // beyond the global top-K still surfaces for the meetings the caller
    // actually asked about (`semantic_search` only takes one `session_id`,
    // not a list). A large filter falls back to one global query plus a
    // post-hoc filter below, bounding the number of embed calls per request.
    let hits: Vec<tauri_plugin_embedding_search::SearchHit> = match meeting_ids {
        Some(ids) if !ids.is_empty() && ids.len() <= MAX_MEETING_ID_FANOUT => {
            let mut merged = Vec::new();
            for id in ids {
                match runtime
                    .semantic_search(query.to_string(), SEMANTIC_KNN_K, Some(id.clone()))
                    .await
                {
                    Ok(hits) => merged.extend(hits),
                    Err(e) => tracing::warn!("semantic search failed for meeting {id}: {e}"),
                }
            }
            merged.sort_by(|a, b| {
                a.distance
                    .partial_cmp(&b.distance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            merged
        }
        _ => match runtime
            .semantic_search(query.to_string(), SEMANTIC_KNN_K, None)
            .await
        {
            Ok(hits) => hits,
            Err(e) => {
                tracing::warn!("semantic search failed: {e}");
                return (
                    ranked,
                    by_session,
                    Some(format!("semantic search failed: {e}")),
                );
            }
        },
    };

    for hit in hits {
        if let Some(filter_ids) = meeting_ids
            && !filter_ids.contains(&hit.session_id)
        {
            continue;
        }
        let start_ms = hit.start_ms.unwrap_or(0);
        let citation = format!("[[src:{}@{start_ms}]]", hit.session_id);
        ranked.push(RankedItem::new(hit.session_id.clone()));
        by_session
            .entry(hit.session_id)
            .or_default()
            .push(SemanticContentMatch {
                chunk_id: hit.chunk_id,
                source_type: hit.source_type,
                text: hit.text,
                distance: hit.distance,
                citation,
            });
    }
    (ranked, by_session, None)
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
        description = "Hybrid content search across meeting notes and transcripts: fuses lexical (SQL LIKE) matches with on-device semantic (embedding KNN) matches via reciprocal rank fusion (k=60), grouped by meeting. Falls back to lexical-only — and says so in `mode` — when the semantic model isn't loaded; never errors solely because semantic search is unavailable. Each match carries a `[[src:<sessionId>@<ms>]]` citation id matching chat's citation format.",
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
        let limit = params.limit.unwrap_or(20).max(1);

        if params.query.trim().is_empty() {
            return structured(&SearchMeetingContentResult {
                query: params.query,
                mode: "none (empty query)".to_string(),
                results: Vec::new(),
            });
        }
        let pattern = like_pattern(&params.query);

        // ---- Lexical arm: documents then transcripts, SQL LIKE. ----
        let doc_rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT session_id, kind, title, body FROM session_documents WHERE deleted_at IS NULL AND (title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\') LIMIT 100",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(internal_err)?;

        let transcript_rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT session_id, memo, words_json FROM transcripts WHERE deleted_at IS NULL AND (memo LIKE ?1 ESCAPE '\\' OR words_json LIKE ?1 ESCAPE '\\') LIMIT 100",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(internal_err)?;

        let mut lexical_ranked: Vec<RankedItem> = Vec::new();
        let mut best_lexical: HashMap<String, LexicalContentMatch> = HashMap::new();

        for (session_id, kind, title, body) in doc_rows {
            if let Some(filter_ids) = &params.meeting_ids
                && !filter_ids.contains(&session_id)
            {
                continue;
            }
            lexical_ranked.push(RankedItem::new(session_id.clone()));
            best_lexical
                .entry(session_id.clone())
                .or_insert_with(|| LexicalContentMatch {
                    source: format!("document:{kind}:{title}"),
                    snippet: extract_snippet(&body, &params.query),
                    score: 1.0,
                    citation: format!("[[src:{session_id}@0]]"),
                });
        }

        for (session_id, memo, words_json) in transcript_rows {
            if let Some(filter_ids) = &params.meeting_ids
                && !filter_ids.contains(&session_id)
            {
                continue;
            }
            lexical_ranked.push(RankedItem::new(session_id.clone()));
            let text = if memo.is_empty() { &words_json } else { &memo };
            best_lexical
                .entry(session_id.clone())
                .or_insert_with(|| LexicalContentMatch {
                    source: "transcript".to_string(),
                    snippet: extract_snippet(text, &params.query),
                    score: 0.8,
                    citation: format!("[[src:{session_id}@0]]"),
                });
        }

        // ---- Semantic arm: on-device embedding KNN, best-effort. A missing
        // model, an unregistered plugin, or a search failure all degrade to
        // lexical-only rather than erroring the whole tool call. ----
        let runtime = self
            .app_handle
            .as_ref()
            .and_then(|app| app.embedding_search());
        let unavailable_reason = if self.app_handle.is_none() {
            Some("no app context")
        } else {
            None
        };
        let (semantic_ranked, mut semantic_by_session, fallback_reason) = semantic_arm(
            runtime,
            unavailable_reason,
            &params.query,
            params.meeting_ids.as_deref(),
        )
        .await;

        let mode = match &fallback_reason {
            None => "hybrid (lexical + semantic, RRF k=60)".to_string(),
            Some(reason) => format!("lexical only (semantic unavailable: {reason})"),
        };

        let fused = fuse_arms(
            &[("lexical", &lexical_ranked), ("semantic", &semantic_ranked)],
            RRF_K,
        );

        let results: Vec<ContentMatch> = fused
            .into_iter()
            .take(limit)
            .map(|entry| ContentMatch {
                lexical: best_lexical.get(&entry.key).cloned(),
                semantic: semantic_by_session.remove(&entry.key).unwrap_or_default(),
                meeting_id: entry.key,
                rrf_score: entry.score,
            })
            .collect();

        let output = SearchMeetingContentResult {
            query: params.query,
            mode,
            results,
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
