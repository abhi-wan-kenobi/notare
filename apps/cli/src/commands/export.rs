use std::path::Path;

use sqlx::SqlitePool;

use crate::cli::{ExportCommands, ExportDocFormat, ExportTableFormat, ExportTranscriptFormat};
use crate::error::{CliError, CliResult};
use crate::output;

pub async fn run(pool: &SqlitePool, command: ExportCommands) -> CliResult<()> {
    match command {
        ExportCommands::Meeting { id, format, output } => {
            meeting(pool, &id, format, output.as_deref()).await
        }
        ExportCommands::Meetings { format, output } => {
            meetings(pool, format, output.as_deref()).await
        }
        ExportCommands::Transcript {
            meeting,
            format,
            output,
        } => transcript(pool, &meeting, format, output.as_deref()).await,
        ExportCommands::Notes {
            meeting,
            format,
            output,
        } => notes(pool, &meeting, format, output.as_deref()).await,
        ExportCommands::Chat {
            meeting,
            format,
            output,
        } => chat(pool, &meeting, format, output.as_deref()).await,
        ExportCommands::Humans { format, output } => humans(pool, format, output.as_deref()).await,
        ExportCommands::Orgs { format, output } => orgs(pool, format, output.as_deref()).await,
    }
}

// -- meeting (composite) -----------------------------------------------------

async fn meeting(
    pool: &SqlitePool,
    id: &str,
    format: ExportDocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let meeting = hypr_db_app::get_meeting(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("get meeting", e.to_string()))?
        .ok_or_else(|| CliError::not_found(format!("meeting '{id}'"), None))?;

    let participants = hypr_db_app::list_meeting_participants(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("list participants", e.to_string()))?;

    let notes = hypr_db_app::list_notes_by_meeting(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("list notes", e.to_string()))?;

    let words = hypr_db_app::load_words(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("load words", e.to_string()))?;

    let hints = hypr_db_app::load_hints(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("load hints", e.to_string()))?;

    let chat_messages = hypr_db_app::load_chat_messages(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("load chat messages", e.to_string()))?;

    let human_ids: Vec<&str> = participants.iter().map(|p| p.human_id.as_str()).collect();
    let mut participant_names = Vec::new();
    for hid in &human_ids {
        let name = hypr_db_app::get_human(pool, hid)
            .await
            .ok()
            .flatten()
            .map(|h| h.name)
            .unwrap_or_else(|| hid.to_string());
        participant_names.push(name);
    }

    let segments = build_segments(&words, &hints);

    match format {
        ExportDocFormat::Json => {
            let value = serde_json::json!({
                "id": meeting.id,
                "title": meeting.title,
                "created_at": meeting.created_at,
                "participants": participant_names,
                "notes": notes.iter().map(|n| serde_json::json!({
                    "kind": n.kind,
                    "title": n.title,
                    "content": n.content,
                })).collect::<Vec<_>>(),
                "transcript": segments.iter().map(|s| serde_json::json!({
                    "speaker": s.speaker,
                    "start_ms": s.start_ms,
                    "end_ms": s.end_ms,
                    "text": s.text,
                })).collect::<Vec<_>>(),
                "chat": chat_messages.iter().map(|m| serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })).collect::<Vec<_>>(),
            });
            output::write_json(out, &value).await
        }
        ExportDocFormat::Markdown => {
            let mut buf = String::new();
            let title = meeting.title.as_deref().unwrap_or("Untitled");
            buf.push_str(&format!("# {title}\n\n"));
            buf.push_str(&format!("**Date:** {}\n", meeting.created_at));
            if !participant_names.is_empty() {
                buf.push_str(&format!(
                    "**Participants:** {}\n",
                    participant_names.join(", ")
                ));
            }

            if !notes.is_empty() {
                buf.push_str("\n## Notes\n\n");
                for note in &notes {
                    if !note.title.is_empty() {
                        buf.push_str(&format!("### {}\n\n", note.title));
                    }
                    buf.push_str(&note.content);
                    buf.push_str("\n\n");
                }
            }

            if !segments.is_empty() {
                buf.push_str("## Transcript\n\n");
                for seg in &segments {
                    let ts = output::format_timestamp_ms(seg.start_ms);
                    buf.push_str(&format!("**{}** ({ts}): {}\n\n", seg.speaker, seg.text));
                }
            }

            if !chat_messages.is_empty() {
                buf.push_str("## Chat\n\n");
                for msg in &chat_messages {
                    let role = capitalize(&msg.role);
                    buf.push_str(&format!("**{role}:** {}\n\n", msg.content));
                }
            }

            output::write_text(out, buf).await
        }
        ExportDocFormat::Text => {
            let mut buf = String::new();
            let title = meeting.title.as_deref().unwrap_or("Untitled");
            buf.push_str(&format!("{title}\n{}\n\n", meeting.created_at));

            for note in &notes {
                if !note.title.is_empty() {
                    buf.push_str(&format!("[{}]\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }

            for seg in &segments {
                let ts = output::format_timestamp_ms(seg.start_ms);
                buf.push_str(&format!("{} ({ts}): {}\n", seg.speaker, seg.text));
            }

            if !chat_messages.is_empty() {
                buf.push('\n');
                for msg in &chat_messages {
                    buf.push_str(&format!("{}: {}\n", msg.role, msg.content));
                }
            }

            output::write_text(out, buf).await
        }
    }
}

// -- meetings list -----------------------------------------------------------

async fn meetings(
    pool: &SqlitePool,
    format: ExportTableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = hypr_db_app::list_meetings(pool)
        .await
        .map_err(|e| CliError::operation_failed("list meetings", e.to_string()))?;

    match format {
        ExportTableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m.id,
                        "title": m.title,
                        "created_at": m.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportTableFormat::Csv => {
            let mut buf = String::from("id,title,created_at\n");
            for m in &rows {
                let title = csv_escape(m.title.as_deref().unwrap_or(""));
                buf.push_str(&format!("{},{},{}\n", m.id, title, m.created_at));
            }
            output::write_text(out, buf).await
        }
        ExportTableFormat::Text => {
            let mut buf = String::new();
            for m in &rows {
                let title = m.title.as_deref().unwrap_or("");
                buf.push_str(&format!("{}\t{}\t{}\n", m.id, title, m.created_at));
            }
            output::write_text(out, buf).await
        }
    }
}

// -- transcript --------------------------------------------------------------

async fn transcript(
    pool: &SqlitePool,
    meeting_id: &str,
    format: ExportTranscriptFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("load words", e.to_string()))?;

    let hints = hypr_db_app::load_hints(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("load hints", e.to_string()))?;

    let segments = build_segments(&words, &hints);

    match format {
        ExportTranscriptFormat::Json => {
            let value: Vec<_> = segments
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "speaker": s.speaker,
                        "start_ms": s.start_ms,
                        "end_ms": s.end_ms,
                        "text": s.text,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportTranscriptFormat::Text => {
            let mut buf = String::new();
            for seg in &segments {
                let ts = output::format_timestamp_ms(seg.start_ms);
                buf.push_str(&format!("{} ({ts}): {}\n", seg.speaker, seg.text));
            }
            output::write_text(out, buf).await
        }
        ExportTranscriptFormat::Srt => {
            let mut buf = String::new();
            for (i, seg) in segments.iter().enumerate() {
                buf.push_str(&format!("{}\n", i + 1));
                buf.push_str(&format!(
                    "{} --> {}\n",
                    srt_timestamp(seg.start_ms),
                    srt_timestamp(seg.end_ms)
                ));
                if seg.speaker != "Speaker" {
                    buf.push_str(&format!("{}: {}\n\n", seg.speaker, seg.text));
                } else {
                    buf.push_str(&format!("{}\n\n", seg.text));
                }
            }
            output::write_text(out, buf).await
        }
        ExportTranscriptFormat::Vtt => {
            let mut buf = String::from("WEBVTT\n\n");
            for seg in &segments {
                buf.push_str(&format!(
                    "{} --> {}\n",
                    vtt_timestamp(seg.start_ms),
                    vtt_timestamp(seg.end_ms)
                ));
                if seg.speaker != "Speaker" {
                    buf.push_str(&format!("{}: {}\n\n", seg.speaker, seg.text));
                } else {
                    buf.push_str(&format!("{}\n\n", seg.text));
                }
            }
            output::write_text(out, buf).await
        }
    }
}

// -- notes -------------------------------------------------------------------

async fn notes(
    pool: &SqlitePool,
    meeting_id: &str,
    format: ExportDocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = hypr_db_app::list_notes_by_meeting(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("list notes", e.to_string()))?;

    match format {
        ExportDocFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "kind": n.kind,
                        "title": n.title,
                        "content": n.content,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportDocFormat::Markdown => {
            let mut buf = String::new();
            for note in &rows {
                if !note.title.is_empty() {
                    buf.push_str(&format!("## {}\n\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }
            output::write_text(out, buf).await
        }
        ExportDocFormat::Text => {
            let mut buf = String::new();
            for note in &rows {
                if !note.title.is_empty() {
                    buf.push_str(&format!("[{}]\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }
            output::write_text(out, buf).await
        }
    }
}

// -- chat --------------------------------------------------------------------

async fn chat(
    pool: &SqlitePool,
    meeting_id: &str,
    format: ExportDocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = hypr_db_app::load_chat_messages(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("load chat messages", e.to_string()))?;

    match format {
        ExportDocFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                        "created_at": m.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportDocFormat::Markdown => {
            let mut buf = String::new();
            for msg in &rows {
                let role = capitalize(&msg.role);
                buf.push_str(&format!("**{role}:** {}\n\n", msg.content));
            }
            output::write_text(out, buf).await
        }
        ExportDocFormat::Text => {
            let mut buf = String::new();
            for msg in &rows {
                buf.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
            output::write_text(out, buf).await
        }
    }
}

// -- humans ------------------------------------------------------------------

async fn humans(pool: &SqlitePool, format: ExportTableFormat, out: Option<&Path>) -> CliResult<()> {
    let rows = hypr_db_app::list_humans(pool)
        .await
        .map_err(|e| CliError::operation_failed("list humans", e.to_string()))?;

    match format {
        ExportTableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "id": h.id,
                        "name": h.name,
                        "email": h.email,
                        "job_title": h.job_title,
                        "org_id": h.org_id,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportTableFormat::Csv => {
            let mut buf = String::from("id,name,email,job_title,org_id\n");
            for h in &rows {
                buf.push_str(&format!(
                    "{},{},{},{},{}\n",
                    h.id,
                    csv_escape(&h.name),
                    csv_escape(&h.email),
                    csv_escape(&h.job_title),
                    h.org_id,
                ));
            }
            output::write_text(out, buf).await
        }
        ExportTableFormat::Text => {
            let mut buf = String::new();
            for h in &rows {
                buf.push_str(&format!("{}\t{}\t{}\n", h.id, h.name, h.email));
            }
            output::write_text(out, buf).await
        }
    }
}

// -- orgs --------------------------------------------------------------------

async fn orgs(pool: &SqlitePool, format: ExportTableFormat, out: Option<&Path>) -> CliResult<()> {
    let rows = hypr_db_app::list_organizations(pool)
        .await
        .map_err(|e| CliError::operation_failed("list organizations", e.to_string()))?;

    match format {
        ExportTableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|o| {
                    serde_json::json!({
                        "id": o.id,
                        "name": o.name,
                        "created_at": o.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        ExportTableFormat::Csv => {
            let mut buf = String::from("id,name,created_at\n");
            for o in &rows {
                buf.push_str(&format!(
                    "{},{},{}\n",
                    o.id,
                    csv_escape(&o.name),
                    o.created_at,
                ));
            }
            output::write_text(out, buf).await
        }
        ExportTableFormat::Text => {
            let mut buf = String::new();
            for o in &rows {
                buf.push_str(&format!("{}\t{}\n", o.id, o.name));
            }
            output::write_text(out, buf).await
        }
    }
}

// -- helpers -----------------------------------------------------------------

struct Segment {
    speaker: String,
    start_ms: i64,
    end_ms: i64,
    text: String,
}

fn build_segments(
    words: &[hypr_transcript::FinalizedWord],
    hints: &[hypr_db_app::PersistableSpeakerHint],
) -> Vec<Segment> {
    use hypr_transcript::SpeakerHintData;

    let hint_map: std::collections::HashMap<&str, &hypr_db_app::PersistableSpeakerHint> =
        hints.iter().map(|h| (h.word_id.as_str(), h)).collect();

    let mut segments: Vec<Segment> = Vec::new();

    for word in words {
        let speaker = hint_map
            .get(word.id.as_str())
            .map(|h| match &h.data {
                SpeakerHintData::UserSpeakerAssignment { human_id } => human_id.clone(),
                SpeakerHintData::ProviderSpeakerIndex {
                    speaker_index,
                    channel,
                    ..
                } => {
                    let ch = channel.unwrap_or(word.channel);
                    format!("Speaker {ch}-{speaker_index}")
                }
            })
            .unwrap_or_else(|| format!("Channel {}", word.channel));

        if let Some(last) = segments.last_mut() {
            if last.speaker == speaker {
                last.text.push(' ');
                last.text.push_str(&word.text);
                last.end_ms = word.end_ms;
                continue;
            }
        }

        segments.push(Segment {
            speaker,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
            text: word.text.clone(),
        });
    }

    segments
}

fn srt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let f = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02},{f:03}")
}

fn vtt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let f = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{f:03}")
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
