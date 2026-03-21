mod runtime;

use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};
use crate::llm::{LlmOverrides, LlmProvider, resolve_config};

pub struct Args {
    pub meeting: Option<String>,
    pub prompt: String,
    pub provider: Option<LlmProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub pool: SqlitePool,
}

pub async fn run(args: Args) -> CliResult<()> {
    let pool = args.pool;
    let system_message = match args.meeting.as_deref() {
        Some(meeting_id) => Some(load_meeting_context(&pool, meeting_id).await?),
        None => None,
    };
    let config = resolve_config(
        &pool,
        LlmOverrides {
            provider: args.provider,
            base_url: args.base_url,
            api_key: args.api_key,
            model: args.model,
        },
    )
    .await?;

    crate::agent::run_prompt(config, system_message, &args.prompt).await
}

async fn load_meeting_context(pool: &SqlitePool, meeting_id: &str) -> CliResult<String> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("get meeting", e.to_string()))?
        .ok_or_else(|| CliError::not_found(format!("meeting '{meeting_id}'"), None))?;

    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .unwrap_or_default();
    let transcript_text = {
        let mut sorted = words;
        sorted.sort_by_key(|w| w.start_ms);
        let text: String = sorted
            .iter()
            .map(|w| w.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() { None } else { Some(text) }
    };

    let participant_rows = hypr_db_app::list_meeting_participants(pool, meeting_id)
        .await
        .unwrap_or_default();
    let mut participants = Vec::new();
    for row in &participant_rows {
        if let Ok(Some(human)) = hypr_db_app::get_human(pool, &row.human_id).await {
            if !human.name.is_empty() {
                participants.push(hypr_template_cli::Participant {
                    name: human.name,
                    job_title: if human.job_title.is_empty() {
                        None
                    } else {
                        Some(human.job_title)
                    },
                });
            }
        }
    }

    let memo = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .ok()
        .flatten()
        .map(|n| n.content)
        .filter(|v| !v.trim().is_empty());
    let summary = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "summary")
        .await
        .ok()
        .flatten()
        .map(|n| n.content)
        .filter(|v| !v.trim().is_empty());

    if meeting.title.is_none() && memo.is_none() && summary.is_none() && transcript_text.is_none() {
        return Err(CliError::operation_failed(
            "load meeting context",
            format!("meeting '{meeting_id}' has no transcript, memo, or summary"),
        ));
    }

    let ctx = hypr_template_cli::ChatContext {
        meeting_id: meeting_id.to_string(),
        title: meeting.title,
        created_at: Some(meeting.created_at),
        participants,
        memo,
        summary,
        transcript_text,
    };

    ctx.render()
        .map_err(|e| CliError::operation_failed("render meeting context", e.to_string()))
}
