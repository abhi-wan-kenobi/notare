mod fixture;
mod renderer;

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use fixture::Fixture;
use owhisper_interface::stream::StreamResponse;
use ratatui::DefaultTerminal;
use transcript::FlushMode;
use transcript::input::TranscriptInput;
use transcript::view::TranscriptView;

#[derive(clap::Parser)]
#[command(name = "replay", about = "Replay transcript fixture in the terminal")]
struct Args {
    #[arg(short, long, default_value_t = Fixture::Deepgram)]
    fixture: Fixture,

    #[arg(short, long, default_value_t = 30)]
    speed: u64,
}

struct App {
    responses: Vec<StreamResponse>,
    position: usize,
    paused: bool,
    speed_ms: u64,
    view: TranscriptView,
    fixture_name: String,
}

impl App {
    fn new(responses: Vec<StreamResponse>, speed_ms: u64, fixture_name: String) -> Self {
        Self {
            responses,
            position: 0,
            paused: false,
            speed_ms,
            view: TranscriptView::new(),
            fixture_name,
        }
    }

    fn total(&self) -> usize {
        self.responses.len()
    }

    fn seek_to(&mut self, target: usize) {
        let target = target.min(self.total());
        self.view = TranscriptView::new();
        self.position = 0;
        for i in 0..target {
            if let Some(input) = TranscriptInput::from_stream_response(&self.responses[i]) {
                self.view.process(input);
            }
        }
        self.position = target;
    }

    fn advance(&mut self) -> bool {
        if self.position >= self.total() {
            return false;
        }
        if let Some(input) = TranscriptInput::from_stream_response(&self.responses[self.position]) {
            self.view.process(input);
        }
        self.position += 1;
        true
    }

    fn is_done(&self) -> bool {
        self.position >= self.total()
    }
}

fn main() {
    use clap::Parser;
    let args = Args::parse();
    let fixture = args.fixture;
    let speed_ms = args.speed;
    let fixture_name = fixture.to_string();

    let responses: Vec<StreamResponse> =
        serde_json::from_str(fixture.json()).expect("fixture must parse as StreamResponse[]");

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, responses, speed_ms, fixture_name.clone());
    ratatui::restore();

    match result {
        Ok(app) => {
            println!(
                "Done. {} final words from {} events ({} fixture).",
                app.view.frame().final_words.len(),
                app.total(),
                fixture_name,
            );
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn run(
    terminal: &mut DefaultTerminal,
    responses: Vec<StreamResponse>,
    speed_ms: u64,
    fixture_name: String,
) -> std::io::Result<App> {
    let mut app = App::new(responses, speed_ms, fixture_name);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| renderer::render(frame, &app))?;

        let tick_duration = Duration::from_millis(app.speed_ms);
        let elapsed = last_tick.elapsed();
        let timeout = tick_duration.saturating_sub(elapsed);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char(' ') => {
                        app.paused = !app.paused;
                        last_tick = Instant::now();
                    }
                    KeyCode::Right => {
                        app.seek_to(app.position + 1);
                    }
                    KeyCode::Left => {
                        app.seek_to(app.position.saturating_sub(1));
                    }
                    KeyCode::Up => {
                        app.speed_ms = app.speed_ms.saturating_sub(10).max(5);
                    }
                    KeyCode::Down => {
                        app.speed_ms += 10;
                    }
                    KeyCode::Home => {
                        app.seek_to(0);
                    }
                    KeyCode::End => {
                        let total = app.total();
                        app.seek_to(total);
                        app.view.flush(FlushMode::DrainAll);
                    }
                    _ => {}
                }
            }
        } else if !app.paused {
            if last_tick.elapsed() >= tick_duration {
                app.advance();
                last_tick = Instant::now();

                if app.is_done() {
                    app.view.flush(FlushMode::DrainAll);
                    terminal.draw(|frame| renderer::render(frame, &app))?;
                    app.paused = true;
                }
            }
        }
    }

    Ok(app)
}
