use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 8787;
pub const STORE_SCOPE: &str = "mcp-server";
pub const SECRET_SCOPE: &str = "mcp-server";
pub const SECRET_KEY: &str = "bearer-token";

pub fn default_port() -> u16 {
    DEFAULT_PORT
}

#[derive(PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    #[strum(serialize = "enabled")]
    Enabled,
    #[strum(serialize = "port")]
    Port,
}

impl tauri_plugin_store2::ScopedStoreKey for StoreKey {}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpServerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    pub bearer_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpServerSettingsInput {
    pub enabled: bool,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpServerStatus {
    pub running: bool,
    pub port: u16,
    pub error: Option<String>,
}

// Tool parameter schemas (schemars::JsonSchema is required by rmcp)

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListMeetingsParams {
    #[schemars(description = "Optional search query to filter meetings by title or id")]
    pub query: Option<String>,
    #[schemars(description = "Maximum results to return (default 20, max 200)")]
    pub limit: Option<u32>,
    #[schemars(description = "Number of results to skip (default 0)")]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetMeetingParams {
    #[schemars(description = "The meeting id")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetMeetingTranscriptParams {
    #[schemars(description = "The meeting id")]
    pub id: String,
    #[schemars(description = "Word offset to start from (default 0)")]
    pub offset: Option<u32>,
    #[schemars(description = "Maximum number of words (default 200, max 500)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchMeetingsParams {
    #[schemars(description = "Search query string to search meetings with Tantivy")]
    pub query: String,
    #[schemars(description = "Maximum results to return (default 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Offset for pagination (default 0)")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchMeetingContentParams {
    #[schemars(
        description = "Query string to search in meeting notes and transcript content (lexical search)"
    )]
    pub query: String,
    #[schemars(description = "Optional list of meeting ids to restrict search within")]
    pub meeting_ids: Option<Vec<String>>,
    #[schemars(description = "Maximum results to return (default 20)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListRecipesParams {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetDailyNoteParams {
    #[schemars(description = "Date of the daily note in YYYY-MM-DD format")]
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DailyNoteRow {
    pub id: String,
    pub workspace_id: String,
    pub owner_user_id: String,
    pub note_date: String,
    pub body_format: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Best lexical (SQL `LIKE`) hit for one meeting: a document (note/summary)
/// or transcript match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexicalContentMatch {
    pub source: String,
    pub snippet: String,
    pub score: f32,
    /// `[[src:<sessionId>@0]]` — lexical matches have no word-level timing.
    pub citation: String,
}

/// One semantic (embedding KNN) chunk hit for a meeting, closest-first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticContentMatch {
    pub chunk_id: String,
    pub source_type: String,
    pub text: String,
    /// sqlite-vec L2 distance; smaller is closer.
    pub distance: f32,
    /// `[[src:<sessionId>@<startMs>]]`, matching the chat citation format.
    pub citation: String,
}

/// One meeting's fused lexical + semantic result (mirrors `HybridResult` in
/// `apps/desktop/src/search/hybrid.ts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMatch {
    pub meeting_id: String,
    /// Summed RRF score across the arms that matched this meeting.
    pub rrf_score: f64,
    pub lexical: Option<LexicalContentMatch>,
    pub semantic: Vec<SemanticContentMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMeetingContentResult {
    pub query: String,
    /// `"hybrid (lexical + semantic, RRF k=60)"` or a `"lexical only (…)"`
    /// explanation of why the semantic arm did not run.
    pub mode: String,
    pub results: Vec<ContentMatch>,
}
