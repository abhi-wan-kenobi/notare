use std::sync::Arc;

use hypr_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use hypr_listener_core::{RecordingMode, StopSessionParams, TranscriptionMode};
use ractor::Actor;
use tokio::sync::mpsc;

pub use crate::cli::AudioMode;
use crate::config::desktop;
use crate::config::stt::{SttGlobalArgs, resolve_config};
use crate::error::{CliError, CliResult};
use crate::output::format_hhmmss;
use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen, run_screen_inline};

mod action;
mod app;
mod effect;
mod exit;
mod runtime;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::exit::{ExitEvent, ExitScreen, spawn_post_session};
use self::runtime::Runtime;

pub struct Args {
    pub stt: SttGlobalArgs,
    pub record: bool,
    pub audio: AudioMode,
}

const ANIMATION_FRAME: std::time::Duration = std::time::Duration::from_millis(33);
const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

struct Output {
    elapsed: std::time::Duration,
    force_quit: bool,
    segments: Vec<hypr_transcript::Segment>,
}

enum ExternalEvent {
    Listener(runtime::RuntimeEvent),
}

struct ListenScreen {
    app: App,
}

impl ListenScreen {
    fn new() -> Self {
        Self { app: App::new() }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Output> {
        for effect in effects {
            match effect {
                Effect::Exit { force } => {
                    return ScreenControl::Exit(Output {
                        elapsed: self.app.elapsed(),
                        force_quit: force,
                        segments: self.app.segments(),
                    });
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for ListenScreen {
    type ExternalEvent = ExternalEvent;
    type Output = Output;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                let effects = self.app.dispatch(Action::Paste(pasted));
                self.apply_effects(effects)
            }
            TuiEvent::Draw => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::Listener(event) => Action::RuntimeEvent(event),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        format!(
            "char: {} ({})",
            self.app.status(),
            format_hhmmss(self.app.elapsed())
        )
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.has_active_animations() {
            ANIMATION_FRAME
        } else {
            IDLE_FRAME
        }
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let Args {
        stt,
        record,
        audio: audio_mode,
    } = args;

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
    let languages = vec![resolved.language.clone()];

    let session_id = uuid::Uuid::new_v4().to_string();
    let session_label = session_id.clone();
    let vault_base = desktop::resolve_paths().vault_base;

    let (listener_tx, mut listener_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Arc::new(Runtime::new(vault_base.clone(), listener_tx));

    let audio: Arc<dyn hypr_audio_actual::AudioProvider> = match audio_mode {
        AudioMode::Dual => Arc::new(hypr_audio_actual::ActualAudio),
        #[cfg(feature = "dev")]
        AudioMode::Mock => Arc::new(hypr_audio_mock::MockAudio::new(1)),
    };

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio,
        },
    )
    .await
    .map_err(|e| CliError::operation_failed("spawn root actor", e.to_string()))?;

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

    let (external_tx, external_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(event) = listener_rx.recv().await {
            if external_tx.send(ExternalEvent::Listener(event)).is_err() {
                break;
            }
        }
    });

    let output = run_screen(ListenScreen::new(), Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("listen tui", e.to_string()))?;

    if !output.force_quit {
        let session_dir = vault_base.join("sessions").join(&session_label);
        let llm_config = crate::llm::resolve_config(None, None, None, None).ok();

        let (exit_tx, exit_rx) = mpsc::unbounded_channel();
        spawn_post_session(output.segments, session_dir, llm_config, exit_tx);

        let exit_screen = ExitScreen::new(
            session_label,
            output.elapsed,
            vec!["Saving transcript", "Generating summary"],
        );
        let height = exit_screen.viewport_height();
        run_screen_inline(exit_screen, height, Some(exit_rx))
            .await
            .map_err(|e| CliError::operation_failed("exit summary", e.to_string()))?;
    }

    if !output.force_quit {
        let _ = ractor::call!(root_ref, RootMsg::StopSession, StopSessionParams::default());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok(())
}
