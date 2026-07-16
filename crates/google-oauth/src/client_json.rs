use crate::error::Error;

pub const DEFAULT_AUTH_URI: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const DEFAULT_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

/// The OAuth client credentials a user obtains from the Google Cloud console
/// ("Desktop app" OAuth client, downloaded as `client_secret_*.json`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_auth_uri")]
    pub auth_uri: String,
    #[serde(default = "default_token_uri")]
    pub token_uri: String,
}

fn default_auth_uri() -> String {
    DEFAULT_AUTH_URI.to_string()
}

fn default_token_uri() -> String {
    DEFAULT_TOKEN_URI.to_string()
}

/// Which top-level key the client json used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientJsonKind {
    /// `{"installed": {...}}` — Desktop-app OAuth client. This is what we want.
    Installed,
    /// `{"web": {...}}` — Web-app OAuth client. Loopback redirects usually
    /// don't work with these unless the user manually added the redirect URI,
    /// so we accept it with a warning surfaced by the caller.
    Web,
}

#[derive(serde::Deserialize)]
struct RawClientJson {
    installed: Option<RawClientEntry>,
    web: Option<RawClientEntry>,
}

#[derive(serde::Deserialize)]
struct RawClientEntry {
    client_id: Option<String>,
    client_secret: Option<String>,
    auth_uri: Option<String>,
    token_uri: Option<String>,
}

/// Parse a Google OAuth client json (the `client_secret_*.json` download).
///
/// Accepts three shapes:
/// - `{"installed": {...}}` (Desktop app — preferred)
/// - `{"web": {...}}` (Web app — accepted, caller should warn)
/// - a bare `{"client_id": ..., "client_secret": ...}` object
pub fn parse_client_json(json: &str) -> Result<(ClientCredentials, ClientJsonKind), Error> {
    let json = json.trim();
    if json.is_empty() {
        return Err(Error::InvalidClientJson("empty input".to_string()));
    }

    let raw: RawClientJson = serde_json::from_str(json)
        .map_err(|e| Error::InvalidClientJson(format!("not valid JSON: {e}")))?;

    let (entry, kind) = match (raw.installed, raw.web) {
        (Some(installed), _) => (installed, ClientJsonKind::Installed),
        (None, Some(web)) => (web, ClientJsonKind::Web),
        (None, None) => {
            // Maybe it is a bare {client_id, client_secret} object.
            let entry: RawClientEntry = serde_json::from_str(json)
                .map_err(|e| Error::InvalidClientJson(format!("not valid JSON: {e}")))?;
            if entry.client_id.is_none() {
                return Err(Error::InvalidClientJson(
                    "expected an \"installed\" (Desktop app) OAuth client json — \
                     found neither \"installed\" nor \"web\" keys nor a client_id"
                        .to_string(),
                ));
            }
            (entry, ClientJsonKind::Installed)
        }
    };

    let client_id = entry
        .client_id
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| Error::InvalidClientJson("missing client_id".to_string()))?;
    let client_secret = entry
        .client_secret
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| Error::InvalidClientJson("missing client_secret".to_string()))?;

    if !client_id.trim().ends_with(".apps.googleusercontent.com") {
        return Err(Error::InvalidClientJson(format!(
            "client_id does not look like a Google OAuth client id \
             (expected *.apps.googleusercontent.com, got {client_id})"
        )));
    }

    Ok((
        ClientCredentials {
            client_id: client_id.trim().to_string(),
            client_secret: client_secret.trim().to_string(),
            auth_uri: entry
                .auth_uri
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(default_auth_uri),
            token_uri: entry
                .token_uri
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(default_token_uri),
        },
        kind,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLED: &str = r#"{
      "installed": {
        "client_id": "1234-abc.apps.googleusercontent.com",
        "project_id": "my-project",
        "auth_uri": "https://accounts.google.com/o/oauth2/auth",
        "token_uri": "https://oauth2.googleapis.com/token",
        "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
        "client_secret": "GOCSPX-secret",
        "redirect_uris": ["http://localhost"]
      }
    }"#;

    #[test]
    fn parses_installed_json() {
        let (creds, kind) = parse_client_json(INSTALLED).unwrap();
        assert_eq!(kind, ClientJsonKind::Installed);
        assert_eq!(creds.client_id, "1234-abc.apps.googleusercontent.com");
        assert_eq!(creds.client_secret, "GOCSPX-secret");
        assert_eq!(creds.auth_uri, "https://accounts.google.com/o/oauth2/auth");
        assert_eq!(creds.token_uri, "https://oauth2.googleapis.com/token");
    }

    #[test]
    fn parses_web_json() {
        let json = r#"{"web": {
            "client_id": "9-z.apps.googleusercontent.com",
            "client_secret": "GOCSPX-x"
        }}"#;
        let (creds, kind) = parse_client_json(json).unwrap();
        assert_eq!(kind, ClientJsonKind::Web);
        assert_eq!(creds.client_id, "9-z.apps.googleusercontent.com");
        assert_eq!(creds.auth_uri, DEFAULT_AUTH_URI);
        assert_eq!(creds.token_uri, DEFAULT_TOKEN_URI);
    }

    #[test]
    fn prefers_installed_over_web() {
        let json = r#"{
            "web": {"client_id": "web.apps.googleusercontent.com", "client_secret": "w"},
            "installed": {"client_id": "app.apps.googleusercontent.com", "client_secret": "i"}
        }"#;
        let (creds, kind) = parse_client_json(json).unwrap();
        assert_eq!(kind, ClientJsonKind::Installed);
        assert_eq!(creds.client_id, "app.apps.googleusercontent.com");
    }

    #[test]
    fn parses_bare_object() {
        let json = r#"{"client_id": "bare.apps.googleusercontent.com", "client_secret": "s"}"#;
        let (creds, kind) = parse_client_json(json).unwrap();
        assert_eq!(kind, ClientJsonKind::Installed);
        assert_eq!(creds.client_id, "bare.apps.googleusercontent.com");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_client_json("").is_err());
        assert!(parse_client_json("not json").is_err());
        assert!(parse_client_json("{}").is_err());
        assert!(parse_client_json(r#"{"service_account": {}}"#).is_err());
    }

    #[test]
    fn rejects_missing_secret() {
        let json = r#"{"installed": {"client_id": "a.apps.googleusercontent.com"}}"#;
        let err = parse_client_json(json).unwrap_err();
        assert!(err.to_string().contains("client_secret"));
    }

    #[test]
    fn rejects_non_google_client_id() {
        let json = r#"{"installed": {"client_id": "hello", "client_secret": "s"}}"#;
        let err = parse_client_json(json).unwrap_err();
        assert!(err.to_string().contains("googleusercontent"));
    }
}
