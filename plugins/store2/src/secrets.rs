//! OS-keyring backed secret storage ("secure-store").
//!
//! Blocking API — callers on an async runtime should wrap these in
//! `spawn_blocking` (the Tauri commands in this plugin do).

const SECURE_STORE_SUFFIX: &str = "secure-store";

fn secure_store_service(identifier: &str) -> String {
    let identifier = match identifier {
        "com.hyprnote.dev" => "com.anarlog.dev",
        "com.hyprnote.staging" => "com.anarlog.staging",
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "com.anarlog.stable",
        identifier => identifier,
    };

    format!("{identifier}.{SECURE_STORE_SUFFIX}")
}

fn legacy_secret_entry<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Option<keyring::Entry>, String> {
    let legacy_service = format!("{}.{}", app.config().identifier, SECURE_STORE_SUFFIX);
    if legacy_service == secure_store_service(&app.config().identifier) {
        return Ok(None);
    }

    let account = format!("{scope}:{key}");
    keyring::Entry::new(&legacy_service, &account)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn secret_entry<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<keyring::Entry, String> {
    if scope.trim().is_empty() || key.trim().is_empty() {
        return Err("secure-store scope and key must not be empty".to_string());
    }

    let service = secure_store_service(&app.config().identifier);
    let account = format!("{scope}:{key}");
    keyring::Entry::new(&service, &account).map_err(|error| error.to_string())
}

/// Read a secret from the OS keyring (with one-time migration from the
/// legacy hyprnote-era service name).
pub fn get_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let entry = secret_entry(app, scope, key)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => {
            let Some(legacy_entry) = legacy_secret_entry(app, scope, key)? else {
                return Ok(None);
            };
            match legacy_entry.get_password() {
                Ok(secret) => {
                    if entry.set_password(&secret).is_ok() {
                        let _ = legacy_entry.delete_credential();
                    }
                    Ok(Some(secret))
                }
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Write a secret to the OS keyring.
pub fn set_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let entry = secret_entry(app, scope, key)?;
    entry
        .set_password(value)
        .map_err(|error| error.to_string())?;
    if let Some(legacy_entry) = legacy_secret_entry(app, scope, key)? {
        let _ = legacy_entry.delete_credential();
    }
    Ok(())
}

/// Delete a secret from the OS keyring (both current and legacy entries).
pub fn delete_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<(), String> {
    if let Some(legacy_entry) = legacy_secret_entry(app, scope, key)? {
        match legacy_entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    let entry = secret_entry(app, scope, key)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_anarlog_service_names_for_legacy_bundle_identifiers() {
        assert_eq!(
            secure_store_service("com.hyprnote.dev"),
            "com.anarlog.dev.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.staging"),
            "com.anarlog.staging.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.stable"),
            "com.anarlog.stable.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.Hyprnote"),
            "com.anarlog.stable.secure-store"
        );
    }

    #[test]
    fn preserves_unknown_service_identifiers() {
        assert_eq!(
            secure_store_service("com.example.app"),
            "com.example.app.secure-store"
        );
    }
}
