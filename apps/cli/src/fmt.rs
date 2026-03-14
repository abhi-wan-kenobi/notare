use std::time::Duration;

pub fn format_hhmmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub fn format_timestamp_ms(ms: i64) -> String {
    let total_secs = (ms / 1000).max(0);
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins >= 60 {
        let hours = mins / 60;
        let mins = mins % 60;
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

pub fn format_timestamp_secs(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    let frac = ((secs - secs.floor()) * 10.0).round() as u64;
    format!("{mins:02}:{s:02}.{frac}")
}
