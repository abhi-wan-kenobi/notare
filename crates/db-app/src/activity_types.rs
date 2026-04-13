#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ObservationEventRow {
    pub id: String,
    pub observation_id: String,
    pub occurred_at_ms: i64,
    pub event_kind: String,
    pub end_reason: Option<String>,
    pub change_class: Option<String>,
    pub app_id: String,
    pub bundle_id: String,
    pub app_name: String,
    pub activity_kind: String,
    pub window_title: String,
    pub url: String,
    pub domain: String,
    pub text_anchor_identity: String,
    pub observation_key: String,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PreferredObservationAnalysisRow {
    pub observation_id: String,
    pub screenshot_id: String,
    pub screenshot_kind: String,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: String,
    pub summary: String,
}
