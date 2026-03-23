mod capture;

pub use capture::{CaptureLayer, TraceBuffer, new_trace_buffer};

use std::io::{self, Stderr};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::{Terminal, TerminalOptions, Viewport};

pub const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn truncate_line(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Progress,
    Traces,
}

pub struct InlineViewport {
    terminal: Terminal<CrosstermBackend<Stderr>>,
    traces: Option<TraceBuffer>,
    view: View,
    height: u16,
    raw_mode: bool,
}

impl InlineViewport {
    pub fn stderr(height: u16, traces: Option<TraceBuffer>) -> io::Result<Self> {
        let raw_mode = traces.is_some();
        if raw_mode {
            crossterm::terminal::enable_raw_mode()?;
        }
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;
        Ok(Self {
            terminal,
            traces,
            view: View::Progress,
            height,
            raw_mode,
        })
    }

    pub fn poll_toggle(&mut self) {
        if !self.raw_mode {
            return;
        }
        if event::poll(std::time::Duration::ZERO).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            })) = event::read()
            {
                self.view = match self.view {
                    View::Progress => View::Traces,
                    View::Traces => View::Progress,
                };
            }
        }
    }

    pub fn draw(&mut self, lines: &[String]) {
        match self.view {
            View::Progress => self.draw_lines(lines),
            View::Traces => self.draw_traces(),
        }
    }

    fn draw_lines(&mut self, lines: &[String]) {
        let lines: Vec<Line> = lines.iter().map(|s| Line::from(s.as_str())).collect();
        let height = self.height;
        let _ = self.terminal.draw(|frame| {
            let chunks =
                Layout::vertical(vec![Constraint::Length(1); height as usize]).split(frame.area());
            for (i, line) in lines.iter().enumerate() {
                if i < chunks.len() {
                    frame.render_widget(Paragraph::new(line.clone()), chunks[i]);
                }
            }
        });
    }

    fn draw_traces(&mut self) {
        let traces = match self.traces {
            Some(ref buf) => buf,
            None => return,
        };
        let height = self.height as usize;
        let trace_lines: Vec<String> = if let Ok(buf) = traces.lock() {
            buf.iter()
                .rev()
                .take(height.saturating_sub(1))
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        } else {
            vec![]
        };

        let h = self.height;
        let _ = self.terminal.draw(|frame| {
            let chunks =
                Layout::vertical(vec![Constraint::Length(1); h as usize]).split(frame.area());

            let header = Paragraph::new(Line::from("[traces] press 'd' to toggle"))
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(header, chunks[0]);

            for (i, line) in trace_lines.iter().enumerate() {
                if i + 1 < chunks.len() {
                    let p = Paragraph::new(Line::from(line.as_str()))
                        .style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(p, chunks[i + 1]);
                }
            }
        });
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new(""), area);
        })?;
        if self.raw_mode {
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }
}

impl Drop for InlineViewport {
    fn drop(&mut self) {
        if self.raw_mode {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn draw_lines(terminal: &mut Terminal<TestBackend>, lines: &[String], height: u16) {
        let lines: Vec<Line> = lines.iter().map(|s| Line::from(s.as_str())).collect();
        terminal
            .draw(|frame| {
                let chunks = Layout::vertical(vec![Constraint::Length(1); height as usize])
                    .split(frame.area());
                for (i, line) in lines.iter().enumerate() {
                    if i < chunks.len() {
                        frame.render_widget(Paragraph::new(line.clone()), chunks[i]);
                    }
                }
            })
            .unwrap();
    }

    #[test]
    fn render_draws_three_lines() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(3),
            },
        )
        .unwrap();

        let lines = [
            "recording mic  00:05".to_string(),
            "16000 Hz  1 ch  5.0s audio".to_string(),
            "out.wav  lvl ||||....".to_string(),
        ];
        draw_lines(&mut terminal, &lines, 3);

        let buf = terminal.backend().buffer().clone();
        let first_line: String = (0..buf.area.width)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(first_line.contains("recording mic"));
    }
}
