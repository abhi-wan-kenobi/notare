use std::sync::Arc;

use hypr_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use hypr_listener_core::{RecordingMode, StopSessionParams, TranscriptionMode};
use hypr_listener2_core::BatchParams;
use ractor::Actor;
use tokio::sync::mpsc;

use crate::commands::Provider;
use crate::error::{CliError, CliResult};
use crate::runtime::desktop;
use crate::runtime::stt::{ResolvedSttConfig, resolve_config};
use crate::{
    event::{EventHandler, TuiEvent},
    frame::FrameRequester,
    terminal::TerminalGuard,
};

mod app;
mod audio_drop;
mod runtime;
mod ui;
mod waveform;

use app::App;
use audio_drop::AudioDropRequest;
use runtime::{ListenBatchRuntime, ListenRuntime};

pub struct Args {
    pub provider: Provider,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub language: String,
    pub record: bool,
}

fn spawn_batch_transcription(
    request: AudioDropRequest,
    batch_runtime: Arc<ListenBatchRuntime>,
    resolved: &ResolvedSttConfig,
) {
    let batch_session_id = uuid::Uuid::new_v4().to_string();
    let params = build_batch_transcription_params(batch_session_id, request, resolved);

    tokio::spawn(async move {
        let _ = hypr_listener2_core::run_batch(batch_runtime, params).await;
    });
}

fn build_batch_transcription_params(
    session_id: String,
    request: AudioDropRequest,
    resolved: &ResolvedSttConfig,
) -> BatchParams {
    BatchParams {
        session_id,
        provider: resolved.batch_provider(),
        file_path: request.file_path,
        model: resolved.model_option(),
        base_url: resolved.base_url.clone(),
        api_key: resolved.api_key.clone(),
        languages: vec![resolved.language.clone()],
        keywords: vec![],
    }
}

use crate::fmt::format_hhmmss;

pub async fn run(args: Args) -> CliResult<()> {
    let Args {
        provider,
        base_url,
        api_key,
        model,
        language: language_code,
        record,
    } = args;

    let resolved = resolve_config(provider, base_url, api_key, model, language_code).await?;
    let _ = resolved.server.as_ref();
    let languages = vec![resolved.language.clone()];

    let session_id = uuid::Uuid::new_v4().to_string();
    let session_label = session_id.clone();
    let vault_base = desktop::resolve_paths().vault_base;

    let (listener_tx, mut listener_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Arc::new(ListenRuntime::new(vault_base, listener_tx));

    let audio: Arc<dyn hypr_audio_actual::AudioProvider> = Arc::new(hypr_audio_actual::ActualAudio);

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio,
        },
    )
    .await
    .map_err(|e| CliError::external_action_failed("spawn root actor", e.to_string()))?;

    let params = SessionParams {
        session_id,
        languages,
        onboarding: false,
        transcription_mode: TranscriptionMode::Live,
        recording_mode: if record {
            RecordingMode::Disk
        } else {
            RecordingMode::Memory
        },
        model: resolved.model.clone(),
        base_url: resolved.base_url.clone(),
        api_key: resolved.api_key.clone(),
        keywords: vec![],
    };

    ractor::call!(root_ref, RootMsg::StartSession, params)
        .map_err(|e| CliError::operation_failed("start session", e.to_string()))?
        .map_err(|e| CliError::operation_failed("start session", format!("{e:?}")))?;

    let mut terminal = TerminalGuard::new();
    let (draw_tx, draw_rx) = tokio::sync::broadcast::channel(16);
    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel();
    let batch_runtime = Arc::new(ListenBatchRuntime { tx: batch_tx });
    let frame_requester = FrameRequester::new(draw_tx);
    let mut app = App::new(frame_requester.clone());
    let mut events = EventHandler::new(draw_rx);
    events.resume_events();

    frame_requester.schedule_frame();

    loop {
        tokio::select! {
            Some(tui_event) = events.next() => {
                match tui_event {
                    TuiEvent::Key(key) => app.handle_key(key),
                    TuiEvent::Paste(pasted) => {
                        if let Some(request) = app.handle_paste(pasted) {
                            spawn_batch_transcription(
                                request,
                                batch_runtime.clone(),
                                &resolved,
                            );
                        }
                    }
                    TuiEvent::Draw => {
                        terminal
                            .terminal_mut()
                            .draw(|frame| ui::draw(frame, &mut app))
                            .ok();
                        let next_frame = if app.has_active_animations() {
                            std::time::Duration::from_millis(33)
                        } else {
                            std::time::Duration::from_secs(1)
                        };
                        frame_requester.schedule_frame_in(next_frame);
                    }
                }
            }
            Some(listener_event) = listener_rx.recv() => {
                app.handle_listener_event(listener_event);
            }
            Some(batch_event) = batch_rx.recv() => {
                app.handle_batch_event(batch_event);
            }
            else => break,
        }

        if app.should_quit {
            break;
        }
    }

    let elapsed = app.elapsed();
    let force_quit = app.force_quit;

    events.pause_events();
    drop(terminal);

    print_exit_summary(&session_label, elapsed);

    if !force_quit {
        let _ = ractor::call!(root_ref, RootMsg::StopSession, StopSessionParams::default());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok(())
}

fn print_exit_summary(session_id: &str, elapsed: std::time::Duration) {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let bold = "\x1b[1m";
    let cyan = "\x1b[36m";

    println!();
    println!("  {dim}Session{reset}   {session_id}");
    println!("  {dim}Duration{reset}  {}", format_hhmmss(elapsed));
    println!();
    println!("  {dim}Chat with this session:{reset}");
    println!(
        "  {bold}{cyan}char chat --session {session_id} --api-key <KEY> --model <MODEL>{reset}"
    );
    println!();
}
