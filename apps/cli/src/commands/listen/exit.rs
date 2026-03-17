use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::llm::ResolvedLlmConfig;
use crate::output::format_hhmmss;
use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const AUTO_EXIT_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

pub enum ExitEvent {
    TaskStarted(usize),
    TaskDone(usize),
    TaskFailed(usize, String),
    AllDone,
}

enum TaskState {
    Done,
    InProgress,
    NotStarted,
    Failed(String),
}

struct TaskItem {
    label: &'static str,
    state: TaskState,
}

pub struct ExitScreen {
    session_id: String,
    elapsed: std::time::Duration,
    spinner_tick: usize,
    tasks: Vec<TaskItem>,
    done_at: Option<std::time::Instant>,
}

impl ExitScreen {
    pub fn new(
        session_id: String,
        elapsed: std::time::Duration,
        task_labels: Vec<&'static str>,
    ) -> Self {
        let tasks = task_labels
            .into_iter()
            .map(|label| TaskItem {
                label,
                state: TaskState::NotStarted,
            })
            .collect();
        Self {
            session_id,
            elapsed,
            spinner_tick: 0,
            tasks,
            done_at: None,
        }
    }

    pub fn viewport_height(&self) -> u16 {
        let content = 6 + 1 + self.tasks.len() as u16;
        let padding_v = 2;
        let border = 2;
        let outer_margin = 2;
        content + padding_v + border + outer_margin
    }
}

impl Screen for ExitScreen {
    type ExternalEvent = ExitEvent;
    type Output = ();

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => ScreenControl::Exit(()),
                    _ => ScreenControl::Continue,
                }
            }
            TuiEvent::Draw => {
                self.spinner_tick = self.spinner_tick.wrapping_add(1);
                if let Some(done_at) = self.done_at {
                    if done_at.elapsed() >= AUTO_EXIT_DELAY {
                        return ScreenControl::Exit(());
                    }
                }
                ScreenControl::Continue
            }
            _ => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            ExitEvent::TaskStarted(idx) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::InProgress;
                }
            }
            ExitEvent::TaskDone(idx) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::Done;
                }
            }
            ExitEvent::TaskFailed(idx, msg) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::Failed(msg);
                }
            }
            ExitEvent::AllDone => {
                self.done_at = Some(std::time::Instant::now());
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        use ratatui::layout::{Constraint, Layout};
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, BorderType, Padding, Paragraph};

        let dim = Style::default().add_modifier(Modifier::DIM);
        let chat_cmd = format!(
            "char chat --session {} --api-key <KEY> --model <MODEL>",
            self.session_id
        );

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Session   ", dim),
                Span::raw(&self.session_id),
            ]),
            Line::from(vec![
                Span::styled("Duration  ", dim),
                Span::raw(format_hhmmss(self.elapsed)),
            ]),
            Line::raw(""),
            Line::from(Span::styled("Chat with this session:", dim)),
            Line::raw(""),
            Line::from(Span::styled(
                chat_cmd,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::raw(""),
        ];

        let spinner = SPINNER_FRAMES[self.spinner_tick % SPINNER_FRAMES.len()];
        for task in &self.tasks {
            let line = match &task.state {
                TaskState::Done => Line::from(vec![
                    Span::styled("[✓] ", Style::default().fg(Color::Green)),
                    Span::styled(task.label, dim.add_modifier(Modifier::CROSSED_OUT)),
                ]),
                TaskState::InProgress => Line::from(vec![
                    Span::styled(format!("{spinner}  "), Style::default().fg(Color::Yellow)),
                    Span::raw(task.label),
                ]),
                TaskState::NotStarted => Line::from(vec![
                    Span::styled("[ ] ", dim),
                    Span::styled(task.label, dim),
                ]),
                TaskState::Failed(msg) => Line::from(vec![
                    Span::styled("[!] ", Style::default().fg(Color::Red)),
                    Span::styled(task.label, Style::default().fg(Color::Red)),
                    Span::styled(format!(" ({msg})"), dim.fg(Color::Red)),
                ]),
            };
            lines.push(line);
        }

        let area = frame.area();
        let [_, box_area, _] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Max(80),
            Constraint::Length(2),
        ])
        .areas(area);
        let [_, box_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(box_area);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::new(2, 2, 1, 1));
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(80)
    }
}

fn segments_to_markdown(segments: &[hypr_transcript::Segment]) -> String {
    use hypr_transcript::SpeakerLabeler;

    let mut labeler = SpeakerLabeler::from_segments(segments, None);
    let mut out = String::new();

    for segment in segments {
        let speaker = labeler.label_for(&segment.key, None);
        let start_secs = segment
            .words
            .first()
            .map(|w| w.start_ms / 1000)
            .unwrap_or(0);
        let mm = start_secs / 60;
        let ss = start_secs % 60;

        out.push_str(&format!("**{speaker}** ({mm:02}:{ss:02})\n"));

        let text: String = segment
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&text);
        out.push_str("\n\n");
    }

    out
}

pub fn spawn_post_session(
    segments: Vec<hypr_transcript::Segment>,
    session_dir: PathBuf,
    llm_config: Option<ResolvedLlmConfig>,
    tx: mpsc::UnboundedSender<ExitEvent>,
) {
    tokio::spawn(async move {
        // Task 0: save transcript
        let _ = tx.send(ExitEvent::TaskStarted(0));
        let markdown = segments_to_markdown(&segments);
        let transcript_path = session_dir.join("transcript.md");
        match hypr_storage::fs::atomic_write_async(&transcript_path, &markdown).await {
            Ok(()) => {
                let _ = tx.send(ExitEvent::TaskDone(0));
            }
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                return;
            }
        }

        // Task 1: generate summary
        let _ = tx.send(ExitEvent::TaskStarted(1));
        let Some(config) = llm_config else {
            let _ = tx.send(ExitEvent::TaskFailed(1, "LLM not configured".into()));
            let _ = tx.send(ExitEvent::AllDone);
            return;
        };

        let backend = match crate::agent::Backend::new(config, None) {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                return;
            }
        };

        let prompt = format!(
            "Summarize the following meeting transcript in a few concise paragraphs. \
             Focus on key topics, decisions, and action items.\n\n{markdown}"
        );

        match backend
            .stream_text(prompt, vec![], 1, |_chunk| Ok(()))
            .await
        {
            Ok(Some(summary)) => {
                let summary_path = session_dir.join("summary.md");
                match hypr_storage::fs::atomic_write_async(&summary_path, &summary).await {
                    Ok(()) => {
                        let _ = tx.send(ExitEvent::TaskDone(1));
                    }
                    Err(e) => {
                        let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
                    }
                }
            }
            Ok(None) => {
                let _ = tx.send(ExitEvent::TaskFailed(
                    1,
                    "LLM returned empty response".into(),
                ));
            }
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
            }
        }

        let _ = tx.send(ExitEvent::AllDone);
    });
}
