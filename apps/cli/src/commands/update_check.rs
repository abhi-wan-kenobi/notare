pub struct UpdateHandle(tokio::task::JoinHandle<Option<String>>);

impl UpdateHandle {
    pub fn spawn() -> Self {
        Self(tokio::spawn(async {
            let current = option_env!("APP_VERSION").unwrap_or("0.0.0");
            let resp: serde_json::Value = reqwest::Client::new()
                .get("https://api.github.com/repos/fastrepl/char/releases/latest")
                .header("User-Agent", "char-cli")
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            let tag = resp["tag_name"].as_str()?;
            let latest = tag.strip_prefix('v').unwrap_or(tag);
            if is_newer(latest, current) {
                Some(latest.to_string())
            } else {
                None
            }
        }))
    }

    pub async fn result(self) -> Option<String> {
        self.0.await.ok().flatten()
    }
}

fn parse_triple(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.split('-').next().unwrap_or(s);
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_triple(latest), parse_triple(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_detected() {
        assert!(is_newer("1.2.0", "1.1.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn handles_prerelease_suffix() {
        assert!(is_newer("1.1.0-beta", "1.0.0"));
    }
}
