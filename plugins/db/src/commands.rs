use tauri::ipc::Channel;

use crate::{ExecuteProxyResult, ManagedState, QueryEvent, TransactionStatement};

/// Wire shape of [`hypr_db_core::CloudsyncStatus`] for the specta surface —
/// db-core's own type is serde-only and stays that way (it predates specta
/// and is used by non-tauri consumers); this mirror keeps the boundary.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncStatusPayload {
    pub cloudsync_enabled: bool,
    pub extension_loaded: bool,
    pub configured: bool,
    pub running: bool,
    pub network_initialized: bool,
    pub last_sync_downloaded_count: Option<i64>,
    pub last_sync_at_ms: Option<u64>,
    pub has_unsent_changes: Option<bool>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

impl From<hypr_db_core::CloudsyncStatus> for SyncStatusPayload {
    fn from(status: hypr_db_core::CloudsyncStatus) -> Self {
        Self {
            cloudsync_enabled: status.cloudsync_enabled,
            extension_loaded: status.extension_loaded,
            configured: status.configured,
            running: status.running,
            network_initialized: status.network_initialized,
            last_sync_downloaded_count: status.last_sync_downloaded_count,
            last_sync_at_ms: status.last_sync_at_ms,
            has_unsent_changes: status.has_unsent_changes,
            last_error: status.last_error,
            consecutive_failures: status.consecutive_failures,
        }
    }
}

/// The command result for `sync_status`: the live status when sync is built
/// in, or a stub carrying just `cloudsync_enabled: false` otherwise, so the
/// frontend can render one shape in every configuration.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[allow(dead_code)] // exactly one variant is constructed per feature config
pub(crate) enum SyncStatusResult {
    Live(SyncStatusPayload),
    Unavailable,
}

/// Wire shape of a paired peer from the allowlist.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncPeer {
    pub node_id: String,
    pub label: String,
    pub added_at: i64,
    pub last_seen: i64,
}

#[cfg(all(feature = "sync", target_os = "linux"))]
impl From<sync_p2p::Peer> for SyncPeer {
    fn from(peer: sync_p2p::Peer) -> Self {
        Self {
            node_id: peer.node_id.to_z32(),
            label: peer.label,
            added_at: peer.added_at,
            last_seen: peer.last_seen,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_meetings(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::ListMeetingsInput,
) -> Result<hypr_agent_access::MeetingPage, String> {
    hypr_agent_access::list_meetings(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetMeetingInput,
) -> Result<hypr_agent_access::Meeting, String> {
    hypr_agent_access::get_meeting(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting_transcript(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetMeetingTranscriptInput,
) -> Result<hypr_agent_access::TranscriptPage, String> {
    hypr_agent_access::get_meeting_transcript(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_recurring_meeting_history(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetRecurringMeetingHistoryInput,
) -> Result<hypr_agent_access::MeetingPage, String> {
    hypr_agent_access::get_recurring_meeting_history(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    state
        .execute(sql, params)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute_transaction(
    state: tauri::State<'_, ManagedState>,
    statements: Vec<TransactionStatement>,
) -> Result<Vec<u64>, String> {
    state
        .execute_transaction(statements)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute_proxy(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    method: String,
) -> Result<ExecuteProxyResult, String> {
    let method = method
        .parse::<hypr_db_execute::ProxyQueryMethod>()
        .map_err(|error| error.to_string())?;
    state
        .execute_proxy(sql, params, method)
        .await
        .map(|result| ExecuteProxyResult { rows: result.rows })
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_legacy_import_report(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyImportReport, String> {
    crate::import::get_legacy_import_report(state.pool())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_legacy_cleanup_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyCleanupStatus, String> {
    crate::import::get_legacy_cleanup_status(state.pool())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cleanup_legacy_files(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyCleanupResult, String> {
    crate::import::cleanup_legacy_files(state.pool())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn run_legacy_import(
    state: tauri::State<'_, ManagedState>,
    dry_run: bool,
) -> Result<String, String> {
    crate::import::rerun_legacy_import(state.pool(), dry_run)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn subscribe(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    on_event: Channel<QueryEvent>,
) -> Result<hypr_db_reactive::SubscriptionRegistration, String> {
    state
        .subscribe(
            sql,
            params,
            crate::runtime::QueryEventChannel::new(on_event),
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unsubscribe(
    state: tauri::State<'_, ManagedState>,
    subscription_id: String,
) -> Result<(), String> {
    state
        .unsubscribe(&subscription_id)
        .await
        .map_err(|error| error.to_string())
}

// SYNC-5 sync commands. The specta builder's `collect_commands!` macro does
// not accept `#[cfg]` items, so these exist unconditionally and the bodies
// cfg-gate: on any non-sync build every command is a plain "sync is not
// available" error, keeping the specta surface identical across configs so
// generated bindings don't churn between feature sets.

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<SyncStatusResult, String> {
    #[cfg(all(feature = "sync", target_os = "linux"))]
    {
        state
            .sync_status()
            .await
            .map(|status| SyncStatusResult::Live(SyncStatusPayload::from(status)))
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", target_os = "linux")))]
    {
        let _ = &state;
        Ok(SyncStatusResult::Unavailable)
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_trigger(state: tauri::State<'_, ManagedState>) -> Result<i64, String> {
    #[cfg(all(feature = "sync", target_os = "linux"))]
    {
        state.sync_trigger().await.map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", target_os = "linux")))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_list_peers(
    state: tauri::State<'_, ManagedState>,
) -> Result<Vec<SyncPeer>, String> {
    #[cfg(all(feature = "sync", target_os = "linux"))]
    {
        Ok(state
            .sync_list_peers()
            .into_iter()
            .map(SyncPeer::from)
            .collect())
    }
    #[cfg(not(all(feature = "sync", target_os = "linux")))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_this_device(
    state: tauri::State<'_, ManagedState>,
) -> Result<String, String> {
    #[cfg(all(feature = "sync", target_os = "linux"))]
    {
        state.sync_this_device().map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", target_os = "linux")))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}
