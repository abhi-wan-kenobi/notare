mod output;
mod response;
mod screen;

use std::io::IsTerminal;
use std::sync::Arc;

use hypr_cli_tui::run_screen_inline;
use hypr_listener2_core::{BatchErrorCode, BatchEvent};
use tokio::sync::mpsc;

pub use crate::cli::BatchArgs;
use hypr_db_app::{PersistableSpeakerHint, TranscriptDeltaPersist};
use hypr_transcript::{FinalizedWord, SpeakerHintData, WordState};

use crate::cli::OutputFormat;
use crate::config::desktop;
use crate::config::stt::resolve_config;
use crate::config::stt::{ChannelBatchRuntime, SttGlobalArgs};
use crate::error::{CliError, CliResult};

use self::screen::{BatchScreen, BatchScreenEvent, BatchScreenOutput, BatchScreenResult};

fn spawn_bridge(
    mut batch_rx: mpsc::UnboundedReceiver<BatchEvent>,
    screen_tx: mpsc::UnboundedSender<BatchScreenEvent>,
) {
    tokio::spawn(async move {
        let mut batch_response: Option<owhisper_interface::batch::Response> = None;
        let mut streamed_segments: Vec<owhisper_interface::stream::StreamResponse> = Vec::new();
        let mut failure: Option<(BatchErrorCode, String)> = None;

        while let Some(event) = batch_rx.recv().await {
            match event {
                BatchEvent::BatchStarted { .. } => {
                    let _ = screen_tx.send(BatchScreenEvent::Started);
                }
                BatchEvent::BatchCompleted { .. } => {}
                BatchEvent::BatchResponseStreamed {
                    percentage,
                    response: streamed,
                    ..
                } => {
                    streamed_segments.push(streamed);
                    let _ = screen_tx.send(BatchScreenEvent::Progress(percentage));
                }
                BatchEvent::BatchResponse { response: next, .. } => {
                    batch_response = Some(next);
                }
                BatchEvent::BatchFailed { code, error, .. } => {
                    failure = Some((code, error.clone()));
                    let _ = screen_tx.send(BatchScreenEvent::Failed(error));
                }
            }
        }

        let _ = screen_tx.send(BatchScreenEvent::Completed(BatchScreenResult {
            batch_response,
            streamed_segments,
            failure,
        }));
    });
}

pub async fn run(args: BatchArgs, stt: SttGlobalArgs, quiet: bool) -> CliResult<()> {
    let resolved = resolve_config(
        stt.provider,
        stt.base_url,
        stt.api_key,
        stt.model,
        stt.language,
    )
    .await?;
    // keep local server alive for the duration of this scope
    let _ = resolved.server.as_ref();

    let file_path = args.input.path().to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            args.input.path().display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let file_name = args
        .input
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.to_string());

    let (batch_tx, batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });

    let params = resolved.to_batch_params(session_id.clone(), file_path.to_string(), args.keywords);

    let show_progress = !quiet && std::io::stderr().is_terminal();
    let format = args.format;
    let output = args.output;

    let started = std::time::Instant::now();
    let batch_task =
        tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    let (batch_response, streamed_segments, failure) = if show_progress {
        let (screen_tx, screen_rx) = mpsc::unbounded_channel();
        spawn_bridge(batch_rx, screen_tx);

        let screen = BatchScreen::new(file_name.clone(), started);
        let height = screen.viewport_height();
        let screen_output = run_screen_inline(screen, height, Some(screen_rx))
            .await
            .map_err(|e| CliError::operation_failed("batch tui", e.to_string()))?;

        match screen_output {
            BatchScreenOutput::Done(result) => (
                result.batch_response,
                result.streamed_segments,
                result.failure,
            ),
            BatchScreenOutput::Aborted => {
                batch_task.abort();
                return Err(CliError::operation_failed(
                    "batch transcription",
                    "aborted by user",
                ));
            }
        }
    } else {
        let mut batch_rx = batch_rx;
        let mut batch_response: Option<owhisper_interface::batch::Response> = None;
        let mut streamed_segments: Vec<owhisper_interface::stream::StreamResponse> = Vec::new();
        let mut failure: Option<(BatchErrorCode, String)> = None;

        while let Some(event) = batch_rx.recv().await {
            match event {
                BatchEvent::BatchStarted { .. } => {}
                BatchEvent::BatchCompleted { .. } => {}
                BatchEvent::BatchResponseStreamed {
                    response: streamed, ..
                } => {
                    streamed_segments.push(streamed);
                }
                BatchEvent::BatchResponse { response: next, .. } => {
                    batch_response = Some(next);
                }
                BatchEvent::BatchFailed { code, error, .. } => {
                    failure = Some((code, error));
                }
            }
        }

        (batch_response, streamed_segments, failure)
    };

    let result = batch_task
        .await
        .map_err(|e| CliError::operation_failed("batch transcription", e.to_string()))?;
    if let Err(error) = result {
        let message = if let Some((code, message)) = failure {
            format!("{code:?}: {message}")
        } else {
            error.to_string()
        };
        return Err(CliError::operation_failed("batch transcription", message));
    }

    let response = batch_response
        .or_else(|| response::batch_response_from_streams(streamed_segments))
        .ok_or_else(|| {
            CliError::operation_failed("batch transcription", "completed without a final response")
        })?;

    // Persist session to SQLite (non-fatal)
    {
        let db_path = desktop::resolve_paths().vault_base.join("app.db");
        if let Ok(db) = hypr_db_core2::Db3::connect_local_plain(&db_path).await {
            if hypr_db_app::migrate(db.pool()).await.is_ok() {
                let pool = db.pool();
                if hypr_db_app::insert_session(pool, &session_id).await.is_ok() {
                    let (words, hints) = response_to_words(&response);
                    let delta = TranscriptDeltaPersist {
                        new_words: words,
                        hints,
                        replaced_ids: vec![],
                    };
                    let _ = hypr_db_app::apply_delta(pool, &session_id, &delta).await;
                    let _ = hypr_db_app::update_session(
                        pool,
                        &session_id,
                        Some(&file_name),
                        None,
                        None,
                    )
                    .await;
                }
            }
        }
    }

    match format {
        OutputFormat::Json => {
            crate::output::write_json(output.as_deref(), &response).await?;
        }
        OutputFormat::Text => {
            let transcript = output::extract_transcript(&response);
            crate::output::write_text(output.as_deref(), transcript).await?;
        }
        OutputFormat::Pretty => {
            let pretty = output::format_pretty(&response);
            crate::output::write_text(output.as_deref(), pretty).await?;
        }
    }

    if !quiet {
        let elapsed = started.elapsed();
        let audio_duration = response
            .metadata
            .get("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let mut parts = Vec::new();
        if audio_duration > 0.0 {
            parts.push(format!("{:.1}s audio", audio_duration));
        }
        parts.push(format!("in {:.1}s", elapsed.as_secs_f64()));
        if let Some(path) = &output {
            parts.push(format!("-> {}", path.display()));
        }
        use colored::Colorize;
        eprintln!("{}", parts.join(", ").dimmed());
    }

    Ok(())
}

fn response_to_words(
    response: &owhisper_interface::batch::Response,
) -> (Vec<FinalizedWord>, Vec<PersistableSpeakerHint>) {
    let mut words = Vec::new();
    let mut hints = Vec::new();
    for (ch_idx, channel) in response.results.channels.iter().enumerate() {
        let Some(alt) = channel.alternatives.first() else {
            continue;
        };
        for (w_idx, word) in alt.words.iter().enumerate() {
            let word_id = format!("batch-{ch_idx}-{w_idx}");
            words.push(FinalizedWord {
                id: word_id.clone(),
                text: word
                    .punctuated_word
                    .as_deref()
                    .unwrap_or(&word.word)
                    .to_string(),
                start_ms: (word.start * 1000.0) as i64,
                end_ms: (word.end * 1000.0) as i64,
                channel: ch_idx as i32,
                state: WordState::Final,
            });
            if let Some(speaker) = word.speaker {
                hints.push(PersistableSpeakerHint {
                    word_id,
                    data: SpeakerHintData::ProviderSpeakerIndex {
                        speaker_index: speaker as i32,
                        provider: None,
                        channel: Some(ch_idx as i32),
                    },
                });
            }
        }
    }
    (words, hints)
}
