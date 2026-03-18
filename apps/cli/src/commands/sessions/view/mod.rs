mod action;
mod app;
mod effect;
mod ui;

use std::path::PathBuf;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use hypr_transcript::{RuntimeSpeakerHint, WordRef};
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub struct Args {
    pub session_id: String,
    pub db_path: PathBuf,
}

enum ExternalEvent {
    Loaded {
        session: hypr_db_app::SessionRow,
        segments: Vec<hypr_transcript::Segment>,
    },
    LoadError(String),
    Saved,
    SaveError(String),
}

struct ViewScreen {
    app: App,
    external_tx: mpsc::UnboundedSender<ExternalEvent>,
    db_path: PathBuf,
}

impl ViewScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::SaveMemo { session_id, memo } => {
                    let tx = self.external_tx.clone();
                    let db_path = self.db_path.clone();
                    tokio::spawn(async move {
                        match save_memo(db_path, &session_id, &memo).await {
                            Ok(()) => {
                                let _ = tx.send(ExternalEvent::Saved);
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::SaveError(e));
                            }
                        }
                    });
                }
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ViewScreen {
    type ExternalEvent = ExternalEvent;
    type Output = ();

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
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::Loaded { session, segments } => Action::Loaded { session, segments },
            ExternalEvent::LoadError(msg) => Action::LoadError(msg),
            ExternalEvent::Saved => Action::Saved,
            ExternalEvent::SaveError(msg) => Action::SaveError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("view"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    let load_tx = external_tx.clone();
    let session_id = args.session_id.clone();
    let db_path = args.db_path.clone();

    tokio::spawn(async move {
        match load_session_data(db_path, &session_id).await {
            Ok((session, segments)) => {
                let _ = load_tx.send(ExternalEvent::Loaded { session, segments });
            }
            Err(e) => {
                let _ = load_tx.send(ExternalEvent::LoadError(e));
            }
        }
    });

    let screen = ViewScreen {
        app: App::new(args.session_id),
        external_tx,
        db_path: args.db_path,
    };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("view tui", e.to_string()))
}

async fn load_session_data(
    db_path: PathBuf,
    session_id: &str,
) -> Result<(hypr_db_app::SessionRow, Vec<hypr_transcript::Segment>), String> {
    let db = hypr_db_core2::Db3::connect_local_plain(&db_path)
        .await
        .map_err(|e| format!("failed to open database: {e}"))?;

    hypr_db_app::migrate(db.pool())
        .await
        .map_err(|e| format!("migration failed: {e}"))?;

    let session = hypr_db_app::get_session(db.pool(), session_id)
        .await
        .map_err(|e| format!("query failed: {e}"))?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    let words = hypr_db_app::load_words(db.pool(), session_id)
        .await
        .map_err(|e| format!("load words failed: {e}"))?;

    let hints = hypr_db_app::load_hints(db.pool(), session_id)
        .await
        .map_err(|e| format!("load hints failed: {e}"))?;

    let runtime_hints: Vec<RuntimeSpeakerHint> = hints
        .into_iter()
        .map(|h| RuntimeSpeakerHint {
            target: WordRef::FinalWordId(h.word_id),
            data: h.data,
        })
        .collect();

    let segments = hypr_transcript::build_segments(&words, &[], &runtime_hints, None);

    Ok((session, segments))
}

async fn save_memo(db_path: PathBuf, session_id: &str, memo: &str) -> Result<(), String> {
    let db = hypr_db_core2::Db3::connect_local_plain(&db_path)
        .await
        .map_err(|e| format!("failed to open database: {e}"))?;

    hypr_db_app::update_session(db.pool(), session_id, None, None, Some(memo))
        .await
        .map_err(|e| format!("update failed: {e}"))?;

    Ok(())
}
