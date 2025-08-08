use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{palette::tailwind, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, List, ListItem, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Sparkline,
    },
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
            "'{}' not found in '{:?}'",
            args.model,
            owhisper_config::global_config_path()
        ));
    }

    let port = 1234;
    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let server = Server::new(config, Some(port));
    let server_handle =
        tokio::spawn(async move { server.run_with_shutdown(shutdown_signal()).await });

    let mut audio_input = hypr_audio::AudioInput::from_mic(args.device.clone())?;
    let device_name = audio_input.device_name().to_string();

    let client = owhisper_client::ListenClient::builder()
        .api_base(&format!("ws://127.0.0.1:{}", port))
        .api_key(api_key.as_deref().unwrap_or(""))
        .params(owhisper_interface::ListenParams {
            model: Some(args.model.clone()),
            languages: vec![hypr_language::ISO639::En.into()],
            redemption_time_ms: 500,
            ..Default::default()
        })
        .build_single();

    // Shared amplitude data for visualization
    let amplitude_data = Arc::new(Mutex::new(AmplitudeData::new()));
    let amplitude_clone = amplitude_data.clone();

    let mic_stream = audio_input
        .stream()
        .resample(16000)
        .chunks(512)
        .map(move |chunk| {
            // Calculate RMS amplitude for visualization
            let rms = calculate_rms(&chunk);

            // Update amplitude data
            if let Ok(mut data) = amplitude_clone.lock() {
                data.update(rms);
            }

            let amplified: Vec<f32> = chunk.iter().map(|&s| (s * 1.5).clamp(-1.0, 1.0)).collect();
            hypr_audio_utils::f32_to_i16_bytes(amplified)
        });

    let response_stream = client.from_realtime_audio(mic_stream).await?;
    futures_util::pin_mut!(response_stream);

    // Run TUI
    let result = run_tui(response_stream, &args.model, &device_name, amplitude_data).await;

    // Cleanup
    server_handle.abort();
    result
}

fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|&s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

#[derive(Clone)]
struct AmplitudeData {
    current: f32,
    history: VecDeque<f32>,
    peak: f32,
    peak_hold_time: Option<Instant>,
}

impl AmplitudeData {
    fn new() -> Self {
        Self {
            current: 0.0,
            history: VecDeque::from(vec![0.0; 60]), // Keep last 60 samples for sparkline
            peak: 0.0,
            peak_hold_time: None,
        }
    }

    fn update(&mut self, amplitude: f32) {
        self.current = amplitude;

        // Update history for sparkline
        if self.history.len() >= 60 {
            self.history.pop_front();
        }
        self.history.push_back(amplitude);

        // Update peak with hold
        if amplitude > self.peak {
            self.peak = amplitude;
            self.peak_hold_time = Some(Instant::now());
        } else if let Some(hold_time) = self.peak_hold_time {
            if hold_time.elapsed() > Duration::from_secs(2) {
                self.peak = amplitude;
                self.peak_hold_time = None;
            }
        }
    }

    fn get_db(&self) -> f32 {
        if self.current > 0.0 {
            20.0 * self.current.log10()
        } else {
            -60.0
        }
    }

    fn get_normalized_level(&self) -> u16 {
        ((self.current * 100.0).clamp(0.0, 100.0)) as u16
    }

    fn get_sparkline_data(&self) -> Vec<u64> {
        self.history
            .iter()
            .map(|&v| ((v * 100.0).clamp(0.0, 100.0)) as u64)
            .collect()
    }
}

async fn run_tui(
    mut stream: impl futures_util::Stream<Item = owhisper_interface::ListenOutputChunk> + Unpin,
    model: &str,
    device: &str,
    amplitude_data: Arc<Mutex<AmplitudeData>>,
) -> anyhow::Result<()> {
    let mut term = TerminalGuard::new()?;
    let mut state = AppState::new();
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    loop {
        // Get current amplitude
        let amp_data = amplitude_data.lock().unwrap().clone();

        // Draw UI
        term.terminal
            .draw(|f| draw_ui(f, &mut state, model, device, &amp_data))?;

        // Process transcription stream (non-blocking)
        while let Ok(Some(chunk)) =
            tokio::time::timeout(Duration::from_millis(1), stream.next()).await
        {
            state.process_chunk(chunk);
        }

        // Handle keyboard input
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => state.scroll_down(),
                    KeyCode::Up | KeyCode::Char('k') => state.scroll_up(),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        state.clear_transcripts();
                    }
                    _ => {}
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
    transcripts: Vec<TranscriptEntry>,
    start_time: Instant,
    scroll_state: ScrollbarState,
    scroll_position: usize,
    processing: bool,
    last_activity: Instant,
}

#[derive(Clone)]
struct TranscriptEntry {
    text: String,
    timestamp: Instant,
}

impl AppState {
    fn new() -> Self {
        Self {
            transcripts: Vec::new(),
            start_time: Instant::now(),
            scroll_state: ScrollbarState::default(),
            scroll_position: 0,
            processing: false,
            last_activity: Instant::now(),
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

        self.transcripts.push(TranscriptEntry {
            text,
            timestamp: Instant::now(),
        });

        // Keep only last 100 transcripts
        if self.transcripts.len() > 100 {
            self.transcripts.remove(0);
        }

        self.processing = true;
        self.last_activity = Instant::now();

        // Auto-scroll to bottom on new transcript
        self.scroll_position = self.transcripts.len().saturating_sub(1);
        self.update_scroll_state();
    }

    fn clear_transcripts(&mut self) {
        self.transcripts.clear();
        self.scroll_position = 0;
        self.update_scroll_state();
    }

    fn scroll_down(&mut self) {
        self.scroll_position =
            (self.scroll_position + 1).min(self.transcripts.len().saturating_sub(1));
        self.update_scroll_state();
    }

    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
        self.update_scroll_state();
    }

    fn update_scroll_state(&mut self) {
        self.scroll_state = self
            .scroll_state
            .content_length(self.transcripts.len())
            .position(self.scroll_position);
    }

    fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    fn is_active(&self) -> bool {
        self.last_activity.elapsed() < Duration::from_secs(2)
    }
}

fn draw_ui(
    frame: &mut Frame,
    state: &mut AppState,
    model: &str,
    device: &str,
    amplitude_data: &AmplitudeData,
) {
    // Main layout
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Length(7), // Audio visualizer area
        Constraint::Length(3), // Status bar
        Constraint::Min(8),    // Transcripts (smaller, scrollable)
        Constraint::Length(2), // Help
    ])
    .split(frame.area());

    // Header with gradient effect
    draw_header(frame, chunks[0], model);

    // Audio visualization area
    draw_audio_visualizer(frame, chunks[1], amplitude_data);

    // Status bar
    draw_status_bar(frame, chunks[2], state, device);

    // Transcripts with scrollbar
    draw_transcripts(frame, chunks[3], state);

    // Help
    draw_help(frame, chunks[4]);
}

fn draw_header(frame: &mut Frame, area: Rect, model: &str) {
    let header_text = vec![
        Span::styled("🎙 ", Style::default().fg(tailwind::CYAN.c400)),
        Span::styled(
            "Whisper Live",
            Style::default()
                .fg(tailwind::CYAN.c300)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" • ", Style::default().fg(tailwind::SLATE.c600)),
        Span::styled(
            model,
            Style::default()
                .fg(tailwind::BLUE.c400)
                .add_modifier(Modifier::ITALIC),
        ),
    ];

    let header = Paragraph::new(Line::from(header_text))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(tailwind::SLATE.c700))
                .style(Style::default().bg(tailwind::SLATE.c950)),
        );

    frame.render_widget(header, area);
}

fn draw_audio_visualizer(frame: &mut Frame, area: Rect, amplitude_data: &AmplitudeData) {
    let audio_block = Block::default()
        .title(" Audio Input ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tailwind::SLATE.c700))
        .padding(Padding::horizontal(1));

    let inner = audio_block.inner(area);
    frame.render_widget(audio_block, area);

    // Layout for audio components
    let audio_layout = Layout::vertical([
        Constraint::Length(2), // Level gauge
        Constraint::Length(1), // dB display
        Constraint::Length(2), // Sparkline
    ])
    .split(inner);

    // Level gauge with gradient colors
    let level = amplitude_data.get_normalized_level();
    let gauge_color = if level > 80 {
        tailwind::RED.c500
    } else if level > 60 {
        tailwind::YELLOW.c500
    } else if level > 30 {
        tailwind::GREEN.c500
    } else {
        tailwind::BLUE.c600
    };

    let gauge = Gauge::default()
        .block(Block::default().title("Level"))
        .gauge_style(Style::default().fg(gauge_color))
        .percent(level)
        .label(format!("{}%", level))
        .use_unicode(true);

    frame.render_widget(gauge, audio_layout[0]);

    // dB meter
    let db = amplitude_data.get_db();
    let db_text = format!("{:+.1} dB", db);
    let db_color = if db > -10.0 {
        tailwind::RED.c400
    } else if db > -20.0 {
        tailwind::YELLOW.c400
    } else {
        tailwind::GREEN.c400
    };

    let db_display = Paragraph::new(db_text)
        .style(Style::default().fg(db_color))
        .alignment(Alignment::Center);

    frame.render_widget(db_display, audio_layout[1]);

    // Sparkline for waveform history
    let sparkline_data = amplitude_data.get_sparkline_data();
    let sparkline = Sparkline::default()
        .block(Block::default().title("Activity"))
        .data(&sparkline_data)
        .style(Style::default().fg(tailwind::CYAN.c600))
        .max(100);

    frame.render_widget(sparkline, audio_layout[2]);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, state: &AppState, device: &str) {
    let elapsed = state.elapsed();

    // Activity indicator
    let activity = if state.is_active() {
        Span::styled("● ", Style::default().fg(tailwind::GREEN.c400))
    } else {
        Span::styled("○ ", Style::default().fg(tailwind::SLATE.c600))
    };

    let status_items = vec![
        activity,
        Span::styled("Device: ", Style::default().fg(tailwind::SLATE.c500)),
        Span::styled(device, Style::default().fg(tailwind::BLUE.c400)),
        Span::styled(" │ ", Style::default().fg(tailwind::SLATE.c700)),
        Span::styled("Time: ", Style::default().fg(tailwind::SLATE.c500)),
        Span::styled(
            format!(
                "{:02}:{:02}",
                elapsed.as_secs() / 60,
                elapsed.as_secs() % 60
            ),
            Style::default().fg(tailwind::GREEN.c400),
        ),
        Span::styled(" │ ", Style::default().fg(tailwind::SLATE.c700)),
        Span::styled("Transcripts: ", Style::default().fg(tailwind::SLATE.c500)),
        Span::styled(
            state.transcripts.len().to_string(),
            Style::default().fg(tailwind::PURPLE.c400),
        ),
    ];

    let status = Paragraph::new(Line::from(status_items))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(tailwind::SLATE.c700))
                .style(Style::default().bg(tailwind::SLATE.c950)),
        );

    frame.render_widget(status, area);
}

fn draw_transcripts(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(vec![
            Span::styled(" Transcripts ", Style::default().fg(tailwind::SLATE.c400)),
            Span::styled(
                format!(" ({}) ", state.transcripts.len()),
                Style::default().fg(tailwind::SLATE.c600),
            ),
        ])
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tailwind::SLATE.c700))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);

    // Calculate visible area
    let visible_height = inner.height as usize;
    let start_idx = state.scroll_position.saturating_sub(visible_height / 2);
    let end_idx = (start_idx + visible_height).min(state.transcripts.len());

    // Create list items with proper styling
    let items: Vec<ListItem> = state.transcripts[start_idx..end_idx]
        .iter()
        .rev()
        .enumerate()
        .map(|(i, entry)| {
            let age = entry.timestamp.elapsed();
            let style = if age < Duration::from_secs(2) {
                Style::default().fg(tailwind::WHITE)
            } else if age < Duration::from_secs(10) {
                Style::default().fg(tailwind::SLATE.c300)
            } else if age < Duration::from_secs(30) {
                Style::default().fg(tailwind::SLATE.c500)
            } else {
                Style::default().fg(tailwind::SLATE.c600)
            };

            let marker = if i == 0 && state.is_active() {
                Span::styled("▸ ", Style::default().fg(tailwind::CYAN.c400))
            } else {
                Span::styled("  ", Style::default())
            };

            ListItem::new(Line::from(vec![marker, Span::styled(&entry.text, style)]))
        })
        .collect();

    let list = List::new(items);

    frame.render_widget(block, area);
    frame.render_widget(list, inner);

    // Scrollbar
    if state.transcripts.len() > visible_height {
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(symbols::scrollbar::VERTICAL)
                .style(Style::default().fg(tailwind::SLATE.c700))
                .thumb_style(Style::default().fg(tailwind::CYAN.c600)),
            inner.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut state.scroll_state,
        );
    }
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let help_items = vec![
        Span::styled(
            "q",
            Style::default()
                .fg(tailwind::CYAN.c400)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("/", Style::default().fg(tailwind::SLATE.c600)),
        Span::styled(
            "ESC",
            Style::default()
                .fg(tailwind::CYAN.c400)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" quit  ", Style::default().fg(tailwind::SLATE.c500)),
        Span::styled(
            "↑↓",
            Style::default()
                .fg(tailwind::CYAN.c400)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("/", Style::default().fg(tailwind::SLATE.c600)),
        Span::styled(
            "jk",
            Style::default()
                .fg(tailwind::CYAN.c400)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" scroll  ", Style::default().fg(tailwind::SLATE.c500)),
        Span::styled(
            "Ctrl+C",
            Style::default()
                .fg(tailwind::CYAN.c400)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" clear", Style::default().fg(tailwind::SLATE.c500)),
    ];

    let help = Paragraph::new(Line::from(help_items))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(tailwind::SLATE.c800)),
        );

    frame.render_widget(help, area);
}
