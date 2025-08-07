use std::{
    ops::Deref,
    time::{Duration, Instant},
};

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

use futures_util::StreamExt;
use hypr_audio::AsyncSource;

use crate::{misc::shutdown_signal, Server};

#[derive(clap::Parser)]
pub struct RunArgs {
    pub model: String,
    #[arg(short, long)]
    pub config: Option<String>,
    #[arg(short, long)]
    pub device: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn handle_run(args: RunArgs) -> anyhow::Result<()> {
    if args.dry_run {
        return Ok(());
    }

    let config = owhisper_config::Config::new(args.config)?;
    if !config.models.iter().any(|m| m.id() == args.model) {
        return Err(anyhow::anyhow!(
            "'{}' not found in '{}'",
            args.model,
            owhisper_config::Config::global_config_path().display()
        ));
    }

    // Start server
    let port = 1234;
    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let server = Server::new(config, Some(port));
    let server_handle =
        tokio::spawn(async move { server.run_with_shutdown(shutdown_signal()).await });

    // Setup audio input
    let mut audio_input = hypr_audio::AudioInput::from_mic(args.device.clone())?;
    let device_name = audio_input.device_name().to_string();

    // Create whisper client
    let client = owhisper_client::ListenClient::builder()
        .api_base(&format!("ws://127.0.0.1:{}", port))
        .api_key(api_key.as_deref().unwrap_or(""))
        .params(owhisper_interface::ListenParams {
            model: Some(args.model.clone()),
            ..Default::default()
        })
        .build_single();

    let mic_stream = audio_input
        .stream()
        .resample(16000)
        .chunks(1024)
        .map(hypr_audio_utils::f32_to_i16_bytes);

    let response_stream = client.from_realtime_audio(mic_stream).await?;
    futures_util::pin_mut!(response_stream);

    // Run TUI
    let result = run_tui(response_stream, &args.model, &device_name).await;

    // Cleanup
    server_handle.abort();
    result
}

async fn run_tui(
    mut stream: impl futures_util::Stream<Item = owhisper_interface::ListenOutputChunk> + Unpin,
    model: &str,
    device: &str,
) -> anyhow::Result<()> {
    let mut term = TerminalGuard::new()?;
    let mut state = AppState::new();
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    loop {
        // Draw UI
        term.terminal.draw(|f| draw_ui(f, &state, model, device))?;

        // Process transcription stream (non-blocking)
        while let Ok(Some(chunk)) =
            tokio::time::timeout(Duration::from_millis(1), stream.next()).await
        {
            state.process_chunk(chunk);
        }

        // Handle keyboard input
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
            }
        }

        // Update tick
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl Deref for TerminalGuard {
    type Target = Terminal<CrosstermBackend<std::io::Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Clone)]
struct AppState {
    transcripts: Vec<String>,
    start_time: Instant,
}

impl AppState {
    fn new() -> Self {
        Self {
            transcripts: Vec::new(),
            start_time: Instant::now(),
        }
    }

    fn process_chunk(&mut self, chunk: owhisper_interface::ListenOutputChunk) {
        if chunk.words.is_empty() {
            return;
        }

        let text = chunk
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // Just append all text
        self.transcripts.push(text);

        // Keep only last 50 transcripts
        if self.transcripts.len() > 50 {
            self.transcripts.remove(0);
        }
    }

    fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

fn draw_ui(frame: &mut Frame, state: &AppState, model: &str, device: &str) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Length(3), // Status
        Constraint::Min(10),   // Transcripts
        Constraint::Length(1), // Help
    ])
    .split(frame.area());

    // Header
    let header = Paragraph::new(format!("🎤 Whisper Live - {}", model))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    // Status
    let elapsed = state.elapsed();
    let status = Paragraph::new(format!(
        "Device: {} | Time: {:02}:{:02} | Transcripts: {}",
        device,
        elapsed.as_secs() / 60,
        elapsed.as_secs() % 60,
        state.transcripts.len()
    ))
    .style(Style::default().fg(Color::Green))
    .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status, chunks[1]);

    // Transcripts history (newest first)
    let items: Vec<ListItem> = state
        .transcripts
        .iter()
        .rev()
        .take(30) // Show more items since we have more space
        .enumerate()
        .map(|(i, text)| {
            let style = if i == 0 {
                Style::default().fg(Color::White)
            } else if i < 5 {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::styled("▸ ", style),
                Span::styled(text, style),
            ]))
        })
        .collect();

    let transcripts =
        List::new(items).block(Block::default().borders(Borders::ALL).title("Transcripts"));
    frame.render_widget(transcripts, chunks[2]);

    // Help
    let help =
        Paragraph::new("Press 'q' or ESC to quit").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}
