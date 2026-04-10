#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ScreenshotAnalysisRow {
    pub fingerprint: String,
    pub reason: String,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: String,
    pub analysis_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SignalRow {
    pub id: String,
    pub occurred_at_ms: i64,
    pub transition_sequence: i64,
    pub reason: String,
    pub app_id: String,
    pub bundle_id: String,
    pub app_name: String,
    pub activity_kind: String,
    pub window_title: String,
    pub url: String,
    pub domain: String,
    pub content_level: String,
    pub source: String,
    pub text_anchor_identity: String,
    pub fingerprint: String,
    pub payload_json: String,
    pub created_at: String,
}
