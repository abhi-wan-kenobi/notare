mod action;
mod app;
mod effect;
mod ui;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use hypr_transcript::{RuntimeSpeakerHint, WordRef};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub struct Args {
    pub meeting_id: String,
    pub pool: SqlitePool,
}

enum ExternalEvent {
    Loaded {
        meeting: hypr_db_app::MeetingRow,
        segments: Vec<hypr_transcript::Segment>,
        memo: Option<hypr_db_app::NoteRow>,
    },
    LoadError(String),
    Saved,
    SaveError(String),
}

struct ViewScreen {
    app: App,
    external_tx: mpsc::UnboundedSender<ExternalEvent>,
    pool: SqlitePool,
}

impl ViewScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::SaveMemo { meeting_id, memo } => {
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match save_memo(&pool, &meeting_id, &memo).await {
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
            ExternalEvent::Loaded {
                meeting,
                segments,
                memo,
            } => Action::Loaded {
                meeting,
                segments,
                memo,
            },
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
    let meeting_id = args.meeting_id.clone();
    let pool = args.pool.clone();

    tokio::spawn(async move {
        match load_meeting_data(&pool, &meeting_id).await {
            Ok((meeting, segments, memo)) => {
                let _ = load_tx.send(ExternalEvent::Loaded {
                    meeting,
                    segments,
                    memo,
                });
            }
            Err(e) => {
                let _ = load_tx.send(ExternalEvent::LoadError(e));
            }
        }
    });

    let screen = ViewScreen {
        app: App::new(args.meeting_id),
        external_tx,
        pool: args.pool,
    };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("view tui", e.to_string()))
}

async fn load_meeting_data(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<
    (
        hypr_db_app::MeetingRow,
        Vec<hypr_transcript::Segment>,
        Option<hypr_db_app::NoteRow>,
    ),
    String,
> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| format!("query failed: {e}"))?
        .ok_or_else(|| format!("meeting not found: {meeting_id}"))?;

    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .map_err(|e| format!("load words failed: {e}"))?;

    let hints = hypr_db_app::load_hints(pool, meeting_id)
        .await
        .map_err(|e| format!("load hints failed: {e}"))?;

    let memo = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .map_err(|e| format!("load memo failed: {e}"))?;

    let runtime_hints: Vec<RuntimeSpeakerHint> = hints
        .into_iter()
        .map(|h| RuntimeSpeakerHint {
            target: WordRef::FinalWordId(h.word_id),
            data: h.data,
        })
        .collect();

    let segments = hypr_transcript::build_segments(&words, &[], &runtime_hints, None);

    Ok((meeting, segments, memo))
}

async fn save_memo(pool: &SqlitePool, meeting_id: &str, memo: &str) -> Result<(), String> {
    let existing = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .map_err(|e| format!("query failed: {e}"))?;

    match existing {
        Some(note) => {
            hypr_db_app::update_note(pool, &note.id, memo)
                .await
                .map_err(|e| format!("update failed: {e}"))?;
        }
        None => {
            let note_id = format!("{meeting_id}:memo");
            hypr_db_app::insert_note(pool, &note_id, meeting_id, "memo", "", memo)
                .await
                .map_err(|e| format!("insert failed: {e}"))?;
        }
    }

    Ok(())
}
