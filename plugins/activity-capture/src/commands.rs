use std::collections::{HashMap, HashSet};

use crate::{
    ActivityCapturePluginExt,
    events::{
        ActivityCaptureBudget, ActivityCaptureCapabilities, ActivityCaptureScreenshotAnalysis,
        ActivityCaptureSnapshot, ActivityCaptureStatus,
    },
};
use hypr_db_core2::{Db3, DbOpenOptions, DbStorage, MigrationFailurePolicy};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const DAILY_SUMMARY_USER_ID: &str = "local";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivityAppStat {
    pub app_name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivityStats {
    pub signal_count: u32,
    pub screenshot_count: u32,
    pub analysis_count: u32,
    pub unique_app_count: u32,
    pub first_signal_at_ms: Option<i64>,
    pub last_signal_at_ms: Option<i64>,
    pub top_apps: Vec<DailyActivityAppStat>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivityAnalysis {
    pub captured_at_ms: i64,
    pub fingerprint: String,
    pub app_name: String,
    pub window_title: Option<String>,
    pub reason: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailySummaryTopic {
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailySummaryTimelineItem {
    pub time: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StoredDailySummary {
    pub id: String,
    pub date: String,
    pub content: String,
    pub timeline: Vec<DailySummaryTimelineItem>,
    pub topics: Vec<DailySummaryTopic>,
    pub status: String,
    pub source_cursor_ms: i64,
    pub source_fingerprint: String,
    pub generated_at: String,
    pub generation_error: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailySummarySnapshot {
    pub stats: DailyActivityStats,
    pub analyses: Vec<DailyActivityAnalysis>,
    pub summary: Option<StoredDailySummary>,
    pub source_cursor_ms: i64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadDailySummarySnapshotInput {
    pub date: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveDailySummaryInput {
    pub date: String,
    pub content: String,
    pub timeline: Vec<DailySummaryTimelineItem>,
    pub topics: Vec<DailySummaryTopic>,
    pub source_cursor_ms: i64,
    pub source_fingerprint: String,
    pub generated_at: String,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn capabilities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ActivityCaptureCapabilities, String> {
    Ok(app.activity_capture().capabilities())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ActivityCaptureSnapshot>, String> {
    app.activity_capture()
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn latest_screenshot_analysis<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ActivityCaptureScreenshotAnalysis>, String> {
    Ok(app.activity_capture().latest_screenshot_analysis())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ActivityCaptureStatus, String> {
    Ok(app.activity_capture().status().await)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_analyses_in_range<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ActivityCaptureScreenshotAnalysis>, String> {
    app.activity_capture()
        .list_analyses_in_range(start_ms, end_ms)
        .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture()
        .start()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture().stop();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_running<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    Ok(app.activity_capture().is_running())
}

#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigureInput {
    pub budget: Option<ActivityCaptureBudget>,
    pub analyze_screenshots: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    input: ConfigureInput,
) -> Result<(), String> {
    app.activity_capture()
        .configure(input.budget, input.analyze_screenshots)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_daily_summary_snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    input: LoadDailySummarySnapshotInput,
) -> Result<DailySummarySnapshot, String> {
    let activity_db = open_activity_db(&app).await?;
    let app_db = open_app_db(&app).await?;

    let (signals, analyses, screenshot_count) = tokio::try_join!(
        hypr_db_activity::list_signals_in_range(activity_db.pool(), input.start_ms, input.end_ms),
        hypr_db_activity::list_screenshot_analyses_in_range(
            activity_db.pool(),
            input.start_ms,
            input.end_ms
        ),
        hypr_db_activity::count_screenshots_in_range(
            activity_db.pool(),
            input.start_ms,
            input.end_ms
        ),
    )
    .map_err(|error| error.to_string())?;

    let stats = build_daily_activity_stats(&signals, analyses.len() as u32, screenshot_count);
    let analysis_items = analyses
        .iter()
        .map(|analysis| DailyActivityAnalysis {
            captured_at_ms: analysis.captured_at_ms,
            fingerprint: analysis.fingerprint.clone(),
            app_name: analysis.app_name.clone(),
            window_title: if analysis.window_title.is_empty() {
                None
            } else {
                Some(analysis.window_title.clone())
            },
            reason: analysis.reason.clone(),
            summary: analysis.analysis_summary.clone(),
        })
        .collect::<Vec<_>>();

    let source_cursor_ms = signals
        .last()
        .map(|signal| signal.occurred_at_ms)
        .into_iter()
        .chain(analyses.last().map(|analysis| analysis.captured_at_ms))
        .max()
        .unwrap_or_default();
    let source_fingerprint = format!(
        "signals:{}|screenshots:{}|analyses:{}|cursor:{}",
        signals.len(),
        screenshot_count,
        analyses.len(),
        source_cursor_ms
    );

    let summary = hypr_db_app::get_daily_summary_by_date(
        app_db.pool(),
        &input.date,
        &daily_note_id(&input.date),
    )
    .await
    .map_err(|error| error.to_string())?
    .map(map_stored_daily_summary);

    Ok(DailySummarySnapshot {
        stats,
        analyses: analysis_items,
        summary,
        source_cursor_ms,
        source_fingerprint,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn save_daily_summary<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    input: SaveDailySummaryInput,
) -> Result<StoredDailySummary, String> {
    let db = open_app_db(&app).await?;
    let note_id = daily_note_id(&input.date);
    let summary_id = daily_summary_id(&input.date);

    hypr_db_app::get_or_create_daily_note(db.pool(), &note_id, &input.date, DAILY_SUMMARY_USER_ID)
        .await
        .map_err(|error| error.to_string())?;

    hypr_db_app::upsert_daily_summary(
        db.pool(),
        hypr_db_app::UpsertDailySummary {
            id: &summary_id,
            daily_note_id: &note_id,
            date: &input.date,
            content: &input.content,
            timeline_json: &serde_json::to_string(&input.timeline).map_err(|e| e.to_string())?,
            topics_json: &serde_json::to_string(&input.topics).map_err(|e| e.to_string())?,
            status: "ready",
            source_cursor_ms: input.source_cursor_ms,
            source_fingerprint: &input.source_fingerprint,
            generation_error: "",
            generated_at: &input.generated_at,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    let row = hypr_db_app::get_daily_summary(db.pool(), &summary_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "daily summary was not found after save".to_string())?;

    Ok(map_stored_daily_summary(row))
}

async fn open_activity_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Db3, String> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("activity.db");
    Db3::open_with_migrate(
        DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: None,
            migration_failure_policy: MigrationFailurePolicy::Recreate,
        },
        |pool| Box::pin(hypr_db_activity::migrate(pool)),
    )
    .await
    .map_err(|error| error.to_string())
}

async fn open_app_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Db3, String> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("app.db");
    Db3::open_with_migrate(
        DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: None,
            migration_failure_policy: MigrationFailurePolicy::Fail,
        },
        |pool| Box::pin(hypr_db_app::migrate(pool)),
    )
    .await
    .map_err(|error| error.to_string())
}

fn daily_note_id(date: &str) -> String {
    format!("daily-note-{date}")
}

fn daily_summary_id(date: &str) -> String {
    format!("daily-summary-{date}")
}

fn build_daily_activity_stats(
    signals: &[hypr_db_activity::SignalRow],
    analysis_count: u32,
    screenshot_count: u32,
) -> DailyActivityStats {
    let mut counts = HashMap::<String, u32>::new();
    let mut apps = HashSet::<String>::new();

    for signal in signals {
        if signal.app_name.is_empty() {
            continue;
        }
        apps.insert(signal.app_name.clone());
        *counts.entry(signal.app_name.clone()).or_default() += 1;
    }

    let mut top_apps = counts
        .into_iter()
        .map(|(app_name, count)| DailyActivityAppStat { app_name, count })
        .collect::<Vec<_>>();
    top_apps.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.app_name.cmp(&b.app_name))
    });
    top_apps.truncate(5);

    DailyActivityStats {
        signal_count: signals.len() as u32,
        screenshot_count,
        analysis_count,
        unique_app_count: apps.len() as u32,
        first_signal_at_ms: signals.first().map(|signal| signal.occurred_at_ms),
        last_signal_at_ms: signals.last().map(|signal| signal.occurred_at_ms),
        top_apps,
    }
}

fn map_stored_daily_summary(row: hypr_db_app::DailySummaryRow) -> StoredDailySummary {
    StoredDailySummary {
        id: row.id,
        date: row.date,
        content: row.content,
        timeline: serde_json::from_str(&row.timeline_json).unwrap_or_default(),
        topics: serde_json::from_str(&row.topics_json).unwrap_or_default(),
        status: row.status,
        source_cursor_ms: row.source_cursor_ms,
        source_fingerprint: row.source_fingerprint,
        generated_at: row.generated_at,
        generation_error: row.generation_error,
        updated_at: row.updated_at,
    }
}
