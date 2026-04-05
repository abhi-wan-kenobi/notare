use std::{
    fs,
    io::{self, Write},
    ops::RangeInclusive,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use chrono::Local;
use hypr_activity_capture::{Event, Transition};
use serde::Serialize;

use crate::event_row::{DetailField, EventRow};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExportScope {
    Selection,
    Session,
}

impl ExportScope {
    fn label(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::Session => "session",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RawRecord {
    captured_at: SystemTime,
    status: String,
    app_name: String,
    summary: String,
    details: Vec<RawDetailField>,
    raw: RawPayload,
}

impl RawRecord {
    pub(crate) fn from_transition(row: &EventRow, transition: Transition) -> Self {
        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::Transition {
                previous: transition.previous,
                current: transition.current,
            },
        }
    }

    pub(crate) fn placeholder(row: &EventRow, reason: impl Into<String>) -> Self {
        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::Placeholder {
                reason: reason.into(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RawDetailField {
    label: String,
    value: String,
}

impl From<&DetailField> for RawDetailField {
    fn from(detail: &DetailField) -> Self {
        Self {
            label: detail.label.clone(),
            value: detail.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RawPayload {
    Transition {
        previous: Option<Event>,
        current: Option<Event>,
    },
    Placeholder {
        reason: String,
    },
}

#[derive(Debug, Serialize)]
struct ExportBundle {
    exported_at: SystemTime,
    scope: &'static str,
    range: ExportRange,
    records: Vec<ExportedRecord>,
}

#[derive(Debug, Serialize)]
struct ExportRange {
    start_index: usize,
    end_index: usize,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ExportedRecord {
    index: usize,
    #[serde(flatten)]
    record: RawRecord,
}

pub(crate) fn copy_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<usize> {
    let (json, count) = serialize_records(records, range, scope)?;

    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err(io::Error::other("clipboard stdin is unavailable"));
    };
    stdin.write_all(json.as_bytes())?;
    drop(stdin);

    let status = child.wait()?;
    if status.success() {
        Ok(count)
    } else {
        Err(io::Error::other(format!(
            "pbcopy failed with status {status}"
        )))
    }
}

pub(crate) fn save_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<PathBuf> {
    let (json, _) = serialize_records(records, range, scope)?;
    let file_name = format!(
        "activity-capture-{}-{}.json",
        scope.label(),
        Local::now().format("%Y%m%d-%H%M%S")
    );

    let directory = export_directory()?;
    fs::create_dir_all(&directory)?;
    let path = unique_path(&directory, &file_name);
    fs::write(&path, json)?;
    Ok(path)
}

fn serialize_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<(String, usize)> {
    let start = *range.start();
    let end = *range.end();
    let slice = records
        .get(start..=end)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid export range"))?;

    let bundle = ExportBundle {
        exported_at: SystemTime::now(),
        scope: scope.label(),
        range: ExportRange {
            start_index: start,
            end_index: end,
            count: slice.len(),
        },
        records: slice
            .iter()
            .enumerate()
            .map(|(offset, record)| ExportedRecord {
                index: start + offset,
                record: record.clone(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&bundle)
        .map(|json| (json, slice.len()))
        .map_err(io::Error::other)
}

fn unique_path(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("activity-capture-export");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");

    for suffix in 2.. {
        let candidate = directory.join(format!("{stem}-{suffix}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("infinite suffix range should always find a free path")
}

fn export_directory() -> io::Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("out"))
}
