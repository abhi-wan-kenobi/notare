use std::future::Future;

#[derive(Debug, Clone)]
pub struct ObservationEventRow {
    pub id: String,
    pub observation_id: String,
    pub occurred_at_ms: i64,
    pub event_kind: String,
    pub app_name: String,
    pub window_title: String,
}

#[derive(Debug, Clone)]
pub struct ObservationAnalysisRow {
    pub observation_id: String,
    pub screenshot_id: String,
    pub screenshot_kind: String,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct DailySummaryRow {
    pub id: String,
    pub date: String,
    pub content: String,
    pub timeline_json: String,
    pub topics_json: String,
    pub status: String,
    pub source_cursor_ms: i64,
    pub source_fingerprint: String,
    pub generated_at: String,
    pub generation_error: String,
    pub updated_at: String,
}

pub struct InsertObservationEvent {
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
}

pub struct InsertObservationAnalysis {
    pub id: String,
    pub observation_id: String,
    pub screenshot_id: String,
    pub screenshot_kind: String,
    pub captured_at_ms: i64,
    pub model_name: String,
    pub prompt_version: String,
    pub app_name: String,
    pub window_title: String,
    pub summary: String,
}

pub struct InsertScreenshot {
    pub id: String,
    pub observation_id: String,
    pub screenshot_kind: String,
    pub scheduled_at_ms: i64,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub sha256: String,
    pub image_blob: Vec<u8>,
    pub snapshot_json: String,
}

pub struct UpsertDailySummary {
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
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct StorageError {
    pub message: String,
}

impl StorageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait ActivityCaptureStorage: Send + Sync + 'static {
    fn list_observation_events_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> impl Future<Output = Result<Vec<ObservationEventRow>, StorageError>> + Send;

    fn list_preferred_observation_analyses_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> impl Future<Output = Result<Vec<ObservationAnalysisRow>, StorageError>> + Send;

    fn count_screenshots_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> impl Future<Output = Result<u32, StorageError>> + Send;

    fn count_screenshots_since(
        &self,
        since_ms: i64,
    ) -> impl Future<Output = Result<u32, StorageError>> + Send;

    fn total_screenshot_storage_bytes(
        &self,
    ) -> impl Future<Output = Result<i64, StorageError>> + Send;

    fn get_daily_summary_by_date(
        &self,
        date: &str,
        daily_note_id: &str,
    ) -> impl Future<Output = Result<Option<DailySummaryRow>, StorageError>> + Send;

    fn get_daily_summary(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<DailySummaryRow>, StorageError>> + Send;

    fn get_or_create_daily_note(
        &self,
        note_id: &str,
        date: &str,
        user_id: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn upsert_daily_summary(
        &self,
        input: UpsertDailySummary,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn insert_observation_event(
        &self,
        input: InsertObservationEvent,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn insert_observation_analysis(
        &self,
        input: InsertObservationAnalysis,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn insert_screenshot(
        &self,
        input: InsertScreenshot,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}

pub struct NoopStorage;

impl ActivityCaptureStorage for NoopStorage {
    async fn list_observation_events_in_range(
        &self,
        _start_ms: i64,
        _end_ms: i64,
    ) -> Result<Vec<ObservationEventRow>, StorageError> {
        Ok(vec![])
    }

    async fn list_preferred_observation_analyses_in_range(
        &self,
        _start_ms: i64,
        _end_ms: i64,
    ) -> Result<Vec<ObservationAnalysisRow>, StorageError> {
        Ok(vec![])
    }

    async fn count_screenshots_in_range(
        &self,
        _start_ms: i64,
        _end_ms: i64,
    ) -> Result<u32, StorageError> {
        Ok(0)
    }

    async fn count_screenshots_since(&self, _since_ms: i64) -> Result<u32, StorageError> {
        Ok(0)
    }

    async fn total_screenshot_storage_bytes(&self) -> Result<i64, StorageError> {
        Ok(0)
    }

    async fn get_daily_summary_by_date(
        &self,
        _date: &str,
        _daily_note_id: &str,
    ) -> Result<Option<DailySummaryRow>, StorageError> {
        Ok(None)
    }

    async fn get_daily_summary(&self, _id: &str) -> Result<Option<DailySummaryRow>, StorageError> {
        Ok(None)
    }

    async fn get_or_create_daily_note(
        &self,
        _note_id: &str,
        _date: &str,
        _user_id: &str,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    async fn upsert_daily_summary(&self, _input: UpsertDailySummary) -> Result<(), StorageError> {
        Ok(())
    }

    async fn insert_observation_event(
        &self,
        _input: InsertObservationEvent,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    async fn insert_observation_analysis(
        &self,
        _input: InsertObservationAnalysis,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    async fn insert_screenshot(&self, _input: InsertScreenshot) -> Result<(), StorageError> {
        Ok(())
    }
}
