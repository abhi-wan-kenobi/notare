#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailySummaryRow {
    pub id: String,
    pub daily_note_id: String,
    pub date: String,
    pub content: String,
    pub timeline_json: String,
    pub topics_json: String,
    pub status: String,
    pub source_cursor_ms: i64,
    pub source_fingerprint: String,
    pub generation_error: String,
    pub generated_at: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UpsertDailySummary<'a> {
    pub id: &'a str,
    pub daily_note_id: &'a str,
    pub date: &'a str,
    pub content: &'a str,
    pub timeline_json: &'a str,
    pub topics_json: &'a str,
    pub status: &'a str,
    pub source_cursor_ms: i64,
    pub source_fingerprint: &'a str,
    pub generation_error: &'a str,
    pub generated_at: &'a str,
}
