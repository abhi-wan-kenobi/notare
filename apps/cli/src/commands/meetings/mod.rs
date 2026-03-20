pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
pub(crate) mod live;
mod runtime;
mod screen;
pub(crate) mod ui;
pub(crate) mod view;

use clap::Subcommand;
use hypr_cli_tui::run_screen;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::cli::Provider;
use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Start a new meeting
    New {
        #[arg(short = 'p', long, value_enum)]
        provider: Option<Provider>,

        /// Create meeting from an audio file instead of live transcription
        #[arg(long, value_name = "FILE")]
        audio: Option<clio::InputPath>,

        /// Keywords to boost transcription accuracy (with --audio)
        #[arg(long = "keyword", short = 'k', value_name = "KEYWORD")]
        keywords: Vec<String>,
    },
    /// View a specific meeting
    View {
        #[arg(long)]
        id: String,
    },
    /// List participants in a meeting
    Participants {
        #[arg(long)]
        id: String,
    },
    /// Add a participant to a meeting
    AddParticipant {
        #[arg(long)]
        meeting: String,
        #[arg(long)]
        human: String,
    },
    /// Remove a participant from a meeting
    RmParticipant {
        #[arg(long)]
        meeting: String,
        #[arg(long)]
        human: String,
    },
}

use self::app::App;
use self::runtime::Runtime;
use self::screen::MeetingsScreen;

pub async fn run(pool: SqlitePool) -> CliResult<Option<String>> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    let runtime = Runtime::new(pool, external_tx);
    runtime.load_meetings();
    runtime.load_events();

    let screen = MeetingsScreen::new(App::new());

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("meetings tui", e.to_string()))
}

pub async fn new_from_audio(
    input: clio::InputPath,
    stt: crate::config::stt::SttGlobalArgs,
    keywords: Vec<String>,
    pool: SqlitePool,
) -> CliResult<()> {
    use hypr_cli_tui::run_screen_inline;
    use tokio::sync::mpsc;

    use crate::commands::exit::ExitScreen;
    use crate::commands::meetings::live::post_meeting::spawn_post_meeting;
    use crate::commands::transcribe;

    let result = transcribe::run_batch(&input, stt, keywords, false).await?;
    let meeting_id = uuid::Uuid::new_v4().to_string();
    let (words, hints) = transcribe::response_to_words(&result.response);

    let llm_config = crate::llm::resolve_config(&pool, None, None, None, None)
        .await
        .map_err(|e| {
            e.to_string()
                .lines()
                .next()
                .unwrap_or("LLM not configured")
                .to_string()
        });

    let (exit_tx, exit_rx) = mpsc::unbounded_channel();
    spawn_post_meeting(
        llm_config,
        exit_tx,
        words,
        hints,
        String::new(),
        meeting_id.clone(),
        None,
        pool,
    );

    let exit_screen = ExitScreen::new(
        meeting_id,
        result.elapsed,
        vec!["Saving to database", "Generating summary"],
    );
    let height = exit_screen.viewport_height();
    run_screen_inline(exit_screen, height, Some(exit_rx))
        .await
        .map_err(|e| CliError::operation_failed("exit summary", e.to_string()))?;
    Ok(())
}

pub async fn participants(pool: &SqlitePool, meeting_id: &str) -> CliResult<()> {
    let rows = hypr_db_app::list_meeting_participants(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("query", e.to_string()))?;

    for row in &rows {
        println!("{}\t{}", row.human_id, row.source);
    }
    Ok(())
}

pub async fn add_participant(pool: &SqlitePool, meeting_id: &str, human_id: &str) -> CliResult<()> {
    hypr_db_app::add_meeting_participant(pool, meeting_id, human_id, "manual")
        .await
        .map_err(|e| CliError::operation_failed("add participant", e.to_string()))?;
    eprintln!("added {human_id} to {meeting_id}");
    Ok(())
}

pub async fn remove_participant(
    pool: &SqlitePool,
    meeting_id: &str,
    human_id: &str,
) -> CliResult<()> {
    hypr_db_app::remove_meeting_participant(pool, meeting_id, human_id)
        .await
        .map_err(|e| CliError::operation_failed("remove participant", e.to_string()))?;
    eprintln!("removed {human_id} from {meeting_id}");
    Ok(())
}
