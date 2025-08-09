mod event;
mod guard;
mod realtime;
mod recorded;
mod state;
mod ui;

use event::*;
use guard::*;
use realtime::*;
use recorded::*;
use state::*;
use ui::*;

use crate::{misc::shutdown_signal, Server};

#[derive(clap::Parser)]
pub struct RunArgs {
    pub model: String,

    /// Audio file path, '-' for stdin, or omit for microphone
    pub file: Option<String>,

    #[arg(short, long)]
    pub config: Option<String>,

    #[arg(short, long)]
    pub device: Option<String>,
}

pub async fn handle_run(args: RunArgs) -> anyhow::Result<()> {
    log::set_max_level(log::LevelFilter::Off);

    let config = owhisper_config::Config::new(args.config.clone())?;
    if !config.models.iter().any(|m| m.id() == args.model) {
        return Err(anyhow::anyhow!(
            "'{}' not found in '{:?}'",
            args.model,
            owhisper_config::global_config_path()
        ));
    }

    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let server = Server::new(config, None);

    let router = server.build_router().await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let port = addr.port();

    let server_handle = tokio::spawn(async move {
        let handle = axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown_signal());
        let _ = handle.await;
    });

    let input_mode = determine_input_mode(&args)?;

    match input_mode {
        InputMode::File(path) => {
            handle_recorded_input(
                AudioSource::File(path),
                args.model.clone(),
                port,
                api_key.clone(),
            )
            .await?;
        }
        InputMode::Stdin => {
            handle_recorded_input(
                AudioSource::Stdin,
                args.model.clone(),
                port,
                api_key.clone(),
            )
            .await?;
        }
        InputMode::Microphone => {
            handle_realtime_input(args.model, args.device, port, api_key).await?;
        }
    }

    server_handle.abort();
    Ok(())
}

enum InputMode {
    File(String),
    Stdin,
    Microphone,
}

fn determine_input_mode(args: &RunArgs) -> anyhow::Result<InputMode> {
    if let Some(file) = &args.file {
        if file == "-" || is_stdin_piped() {
            Ok(InputMode::Stdin)
        } else {
            Ok(InputMode::File(file.clone()))
        }
    } else if is_stdin_piped() {
        Ok(InputMode::Stdin)
    } else {
        Ok(InputMode::Microphone)
    }
}

fn is_stdin_piped() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}
