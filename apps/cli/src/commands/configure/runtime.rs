use sqlx::SqlitePool;
use tokio::sync::mpsc;

use hypr_db_app::CalendarRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarPermissionState {
    NotDetermined,
    Authorized,
    Denied,
}

pub enum RuntimeEvent {
    SettingsLoaded {
        current_stt: Option<String>,
        current_llm: Option<String>,
        stt_providers: Vec<String>,
        llm_providers: Vec<String>,
    },
    CalendarsLoaded(Vec<CalendarRow>),
    CalendarPermissionStatus(CalendarPermissionState),
    CalendarPermissionResult(bool),
    CalendarPermissionReset,
    Saved,
    Error(String),
}

#[cfg(target_os = "macos")]
fn map_auth_status(status: hypr_apple_calendar::CalendarAuthStatus) -> CalendarPermissionState {
    match status {
        hypr_apple_calendar::CalendarAuthStatus::NotDetermined => {
            CalendarPermissionState::NotDetermined
        }
        hypr_apple_calendar::CalendarAuthStatus::Authorized => CalendarPermissionState::Authorized,
        hypr_apple_calendar::CalendarAuthStatus::Denied => CalendarPermissionState::Denied,
    }
}

pub struct Runtime {
    pool: SqlitePool,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub fn new(pool: SqlitePool, tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { pool, tx }
    }

    pub fn load_settings(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let stt_connections = hypr_db_app::list_connections(&pool, "stt")
                .await
                .unwrap_or_default();
            let llm_connections = hypr_db_app::list_connections(&pool, "llm")
                .await
                .unwrap_or_default();

            let all_settings = hypr_db_app::load_all_settings(&pool)
                .await
                .unwrap_or_default();
            let map: std::collections::HashMap<String, String> = all_settings.into_iter().collect();

            let current_stt = map
                .get("current_stt_provider")
                .filter(|v| !v.is_empty())
                .cloned();
            let current_llm = map
                .get("current_llm_provider")
                .filter(|v| !v.is_empty())
                .cloned();

            let stt_providers: Vec<String> =
                stt_connections.into_iter().map(|c| c.provider_id).collect();
            let llm_providers: Vec<String> =
                llm_connections.into_iter().map(|c| c.provider_id).collect();

            let _ = tx.send(RuntimeEvent::SettingsLoaded {
                current_stt,
                current_llm,
                stt_providers,
                llm_providers,
            });
        });
    }

    pub fn save_stt_provider(&self, provider: String) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match hypr_db_app::set_setting(&pool, "current_stt_provider", &provider).await {
                Ok(()) => {
                    let _ = tx.send(RuntimeEvent::Saved);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub fn save_llm_provider(&self, provider: String) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match hypr_db_app::set_setting(&pool, "current_llm_provider", &provider).await {
                Ok(()) => {
                    let _ = tx.send(RuntimeEvent::Saved);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub fn load_calendars(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let connections = hypr_db_app::list_connections(&pool, "cal")
                .await
                .unwrap_or_default();

            let mut all_calendars = Vec::new();
            for conn in connections {
                if let Ok(cals) = hypr_db_app::list_calendars_by_connection(&pool, &conn.id).await {
                    all_calendars.extend(cals);
                }
            }

            let _ = tx.send(RuntimeEvent::CalendarsLoaded(all_calendars));
        });
    }

    pub fn save_calendars(&self, calendars: Vec<CalendarRow>) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            for cal in &calendars {
                if let Err(e) = hypr_db_app::upsert_calendar(
                    &pool,
                    &cal.id,
                    &cal.provider,
                    &cal.connection_id,
                    &cal.tracking_id,
                    &cal.name,
                    &cal.color,
                    &cal.source,
                    cal.enabled,
                )
                .await
                {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                    return;
                }
            }
            let _ = tx.send(RuntimeEvent::Saved);
        });
    }

    pub fn check_permission(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                let state = map_auth_status(hypr_apple_calendar::Handle::authorization_status());
                let _ = tx.send(RuntimeEvent::CalendarPermissionStatus(state));
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = tx.send(RuntimeEvent::Error(
                    "Calendar permissions are only available on macOS".to_string(),
                ));
            }
        });
    }

    pub fn request_permission(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                let granted = hypr_apple_calendar::Handle::request_full_access();
                let _ = tx.send(RuntimeEvent::CalendarPermissionResult(granted));
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = tx.send(RuntimeEvent::CalendarPermissionResult(false));
            }
        });
    }

    pub fn reset_permission(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = tokio::process::Command::new("tccutil")
                .args(["reset", "Calendar"])
                .output()
                .await;
            match result {
                Ok(_) => {
                    let _ = tx.send(RuntimeEvent::CalendarPermissionReset);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                }
            }
        });
    }
}
