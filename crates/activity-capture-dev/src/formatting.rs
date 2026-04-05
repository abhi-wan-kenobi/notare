use std::time::SystemTime;

use chrono::{DateTime, Local};
use url::Url;

pub fn compact(value: &str, limit: usize) -> String {
    let mut collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "-".to_string();
    }
    if collapsed.chars().count() > limit {
        collapsed = collapsed.chars().take(limit).collect::<String>();
        collapsed.push_str("...");
    }
    collapsed
}

pub fn compact_url(value: &str, limit: usize) -> String {
    let compacted = match Url::parse(value) {
        Ok(url) => {
            let mut compacted = match url.host_str() {
                Some(host) => format!("{}://{host}", url.scheme()),
                None => format!("{}:", url.scheme()),
            };

            let mut suffix = url.path().to_string();
            if let Some(query) = url.query() {
                suffix.push('?');
                suffix.push_str(query);
            }
            if !suffix.is_empty() && suffix != "/" {
                compacted.push_str(&suffix);
            }

            compacted
        }
        Err(_) => value.to_string(),
    };

    compact(&compacted, limit)
}

pub fn format_timestamp(time: SystemTime) -> String {
    let local: DateTime<Local> = time.into();
    local.format("%H:%M:%S").to_string()
}
