use tauri::ipc::Channel;

use crate::{ExecuteProxyResult, ManagedState, QueryEvent, TransactionStatement};

/// Live sync status: whether this database has the cr-sqlite engine loaded,
/// why not when a build that has it could not load it, and whether the P2P
/// agent is running.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncStatusPayload {
    pub engine_loaded: bool,
    pub engine_error: Option<String>,
    pub running: bool,
}

/// The command result for `sync_status`: the live status when sync is built
/// in, or `Unavailable` otherwise, so the frontend can render one shape in
/// every configuration.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[allow(dead_code)] // exactly one variant is constructed per feature config
pub(crate) enum SyncStatusResult {
    Live(SyncStatusPayload),
    Unavailable,
}

/// Wire shape of a paired peer from the allowlist.
///
/// `node_id` stays the compact z32 form (the canonical id — pass it back to
/// `sync_remove_peer`); `fingerprint` is the grouped, dashed display form (the
/// same shape `sync_this_device` / `sync_add_peer` use) for the UI to render.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncPeer {
    pub node_id: String,
    pub fingerprint: String,
    pub label: String,
    pub added_at: i64,
    pub last_seen: i64,
}

#[cfg(all(feature = "sync", sync_platform))]
impl From<sync_p2p::Peer> for SyncPeer {
    fn from(peer: sync_p2p::Peer) -> Self {
        Self {
            node_id: peer.node_id.to_z32(),
            fingerprint: sync_p2p::Fingerprint::from_pubkey(&peer.node_id)
                .as_str()
                .to_string(),
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

/// Runtime opt-in: start the sync agent on an already-running app, without a
/// restart. Idempotent — a no-op if sync is already running.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_start(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state.start_sync().await.map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

/// Runtime opt-out: stop the sync agent while the app keeps running, without
/// a restart. Idempotent — a no-op if sync was never started.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_stop(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state.stop_sync().await.map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<SyncStatusResult, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        let status = state.sync_status().await;
        Ok(SyncStatusResult::Live(SyncStatusPayload {
            engine_loaded: status.engine_loaded,
            engine_error: status.engine_error,
            running: status.running,
        }))
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Ok(SyncStatusResult::Unavailable)
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_trigger(state: tauri::State<'_, ManagedState>) -> Result<i64, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .sync_trigger()
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
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
    #[cfg(all(feature = "sync", sync_platform))]
    {
        Ok(state
            .sync_list_peers()
            .await
            .into_iter()
            .map(SyncPeer::from)
            .collect())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
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
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .sync_this_device()
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_add_peer(
    state: tauri::State<'_, ManagedState>,
    fingerprint: String,
    label: String,
) -> Result<String, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .sync_add_peer(fingerprint, label)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = (&state, fingerprint, label);
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_remove_peer(
    state: tauri::State<'_, ManagedState>,
    fingerprint: String,
) -> Result<bool, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .sync_remove_peer(fingerprint)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = (&state, fingerprint);
        Err("sync is not available in this build".to_string())
    }
}

// ---------------------------------------------------------------------------
// Pairing commands (SYNC-6: QR / link pairing)
// ---------------------------------------------------------------------------

/// Result of creating a pairing offer.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingOfferResult {
    /// The `notare://pair#...` link.
    pub link: String,
    /// SVG markup for the QR code encoding the link.
    pub qr_svg: String,
    /// Unix timestamp (seconds) when the offer expires.
    pub expires_at: u64,
}

/// A pending device waiting for confirmation.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingPendingDevice {
    pub device_id: String,
    pub name: String,
    pub sas: String,
    pub expires_at: u64,
}

/// Status of the pairing subsystem.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingStatusResult {
    pub offer_active: bool,
    pub offer_expires_at: Option<u64>,
    pub pending: Vec<PairingPendingDevice>,
}

/// Result of joining via a pairing link.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingJoinResult {
    /// The 6-digit SAS code to verify.
    pub sas: String,
    /// The offerer's device id (z32 compact).
    pub device_id: String,
    /// The offerer's device name.
    pub device_name: String,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn pairing_create_offer(
    state: tauri::State<'_, ManagedState>,
) -> Result<PairingOfferResult, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .pairing_create_offer()
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn pairing_join(
    state: tauri::State<'_, ManagedState>,
    link: String,
) -> Result<PairingJoinResult, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .pairing_join(link)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = (&state, link);
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn pairing_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<PairingStatusResult, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .pairing_status()
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = &state;
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn pairing_confirm(
    state: tauri::State<'_, ManagedState>,
    device_id: String,
    accept: bool,
) -> Result<(), String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .pairing_confirm(device_id, accept)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = (&state, device_id, accept);
        Err("sync is not available in this build".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_revoke_peer(
    state: tauri::State<'_, ManagedState>,
    fingerprint: String,
) -> Result<bool, String> {
    #[cfg(all(feature = "sync", sync_platform))]
    {
        state
            .sync_revoke_peer(fingerprint)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(all(feature = "sync", sync_platform)))]
    {
        let _ = (&state, fingerprint);
        Err("sync is not available in this build".to_string())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type, sqlx::FromRow,
)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub scope: String,
    pub builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpsertRecipeInput {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub scope: String,
    #[serde(default)]
    pub builtin: bool,
}

pub async fn list_recipes_db(pool: &sqlx::SqlitePool) -> Result<Vec<Recipe>, String> {
    sqlx::query_as::<_, Recipe>(
        "SELECT id, title, prompt, scope, builtin, created_at, updated_at FROM recipes ORDER BY builtin DESC, title COLLATE NOCASE ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())
}

pub(crate) async fn upsert_recipe_db(
    pool: &sqlx::SqlitePool,
    input: UpsertRecipeInput,
) -> Result<Recipe, String> {
    let scope = if input.scope == "multi" {
        "multi"
    } else {
        "single"
    };
    sqlx::query_as::<_, Recipe>(
        r#"
        INSERT INTO recipes (id, title, prompt, scope, builtin, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        ON CONFLICT(id) DO UPDATE SET
          title = excluded.title,
          prompt = excluded.prompt,
          scope = excluded.scope,
          builtin = recipes.builtin,
          updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        RETURNING id, title, prompt, scope, builtin, created_at, updated_at
        "#,
    )
    .bind(input.id)
    .bind(input.title)
    .bind(input.prompt)
    .bind(scope)
    .bind(input.builtin)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())
}

pub(crate) async fn delete_recipe_db(pool: &sqlx::SqlitePool, id: String) -> Result<(), String> {
    // Single statement so the built-in check and the delete cannot interleave.
    let deleted = sqlx::query("DELETE FROM recipes WHERE id = ?1 AND builtin = 0")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?
        .rows_affected();
    if deleted > 0 {
        return Ok(());
    }
    let is_builtin = sqlx::query_scalar::<_, bool>("SELECT builtin FROM recipes WHERE id = ?1")
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    match is_builtin {
        Some(true) => Err("Built-in recipes cannot be deleted".to_string()),
        _ => Ok(()),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_recipes(
    state: tauri::State<'_, ManagedState>,
) -> Result<Vec<Recipe>, String> {
    list_recipes_db(state.pool()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn upsert_recipe(
    state: tauri::State<'_, ManagedState>,
    input: UpsertRecipeInput,
) -> Result<Recipe, String> {
    upsert_recipe_db(state.pool(), input).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_recipe(
    state: tauri::State<'_, ManagedState>,
    id: String,
) -> Result<(), String> {
    delete_recipe_db(state.pool(), id).await
}
