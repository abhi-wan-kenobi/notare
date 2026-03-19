use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use rig::message::Message;

use crate::theme::Theme;
use crate::widgets::ScrollViewState;

use super::Role;
use super::action::Action;
use super::effect::Effect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Speaker {
    User,
    Assistant,
    Error,
}

pub(crate) struct VisibleMessage {
    pub(crate) speaker: Speaker,
    pub(crate) content: String,
}

const MAX_HISTORY: usize = 20;

enum StreamState {
    Idle,
    Streaming(String),
}

fn new_editor() -> Editor<Theme> {
    let mut e = Editor::with_styles(Theme::DEFAULT);
    e.set_placeholder(
        "Type a message and press Enter...",
        Theme::DEFAULT.placeholder,
    );
    e
}

pub(crate) struct App {
    model: String,
    meeting: Option<String>,
    meeting_id: String,
    api_history: Vec<Message>,
    max_history: usize,
    transcript: Vec<VisibleMessage>,
    input: Editor<Theme>,
    stream: StreamState,
    last_error: Option<String>,
    started_at: Instant,
    scroll: ScrollViewState,
    autoscroll: bool,
    terminal_title: Option<String>,
    title_requested: bool,
}

impl App {
    pub(crate) fn new(model: String, meeting: Option<String>, meeting_id: String) -> Self {
        Self {
            model,
            meeting,
            meeting_id,
            api_history: Vec::new(),
            max_history: MAX_HISTORY,
            transcript: Vec::new(),
            input: new_editor(),
            stream: StreamState::Idle,
            last_error: None,
            started_at: Instant::now(),
            scroll: ScrollViewState::new(),
            autoscroll: true,
            terminal_title: None,
            title_requested: false,
        }
    }

    pub(crate) fn load_history(&mut self, messages: Vec<hypr_db_app::ChatMessageRow>) {
        for msg in messages {
            let speaker = match msg.role.as_str() {
                "user" => Speaker::User,
                "assistant" => Speaker::Assistant,
                _ => Speaker::Error,
            };
            self.transcript.push(VisibleMessage {
                speaker,
                content: msg.content.clone(),
            });
            match speaker {
                Speaker::User => self.push_api_history(Message::user(msg.content)),
                Speaker::Assistant => self.push_api_history(Message::assistant(msg.content)),
                _ => {}
            }
        }
        if !self.transcript.is_empty() {
            self.title_requested = true;
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::StreamChunk(chunk) => {
                if let StreamState::Streaming(buf) = &mut self.stream {
                    buf.push_str(&chunk);
                }
                if self.autoscroll {
                    self.scroll.scroll_to_bottom();
                }
                Vec::new()
            }
            Action::StreamCompleted(final_text) => self.finish_stream(final_text),
            Action::StreamFailed(error) => self.fail_stream(error),
            Action::TitleGenerated(title) => {
                self.terminal_title = Some(title.clone());
                vec![Effect::UpdateTitle {
                    meeting_id: self.meeting_id.clone(),
                    title,
                }]
            }
        }
    }

    pub(crate) fn title(&self) -> String {
        match &self.terminal_title {
            Some(title) => hypr_cli_tui::terminal_title(Some(title)),
            None => hypr_cli_tui::terminal_title(Some("chat")),
        }
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn meeting(&self) -> Option<&str> {
        self.meeting.as_deref()
    }

    pub(crate) fn status(&self) -> String {
        if let Some(err) = &self.last_error {
            format!("Error: {err}")
        } else if self.streaming() {
            "Streaming response...".into()
        } else {
            "Ready".into()
        }
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub(crate) fn input(&self) -> &Editor<Theme> {
        &self.input
    }

    pub(crate) fn input_mut(&mut self) -> &mut Editor<Theme> {
        &mut self.input
    }

    pub(crate) fn transcript(&self) -> &[VisibleMessage] {
        &self.transcript
    }

    pub(crate) fn pending_assistant(&self) -> &str {
        match &self.stream {
            StreamState::Streaming(buf) => buf,
            StreamState::Idle => "",
        }
    }

    pub(crate) fn streaming(&self) -> bool {
        matches!(self.stream, StreamState::Streaming(_))
    }

    pub(crate) fn apply_autoscroll(&mut self) {
        if self.autoscroll {
            self.scroll.scroll_to_bottom();
        }
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut ScrollViewState {
        &mut self.scroll
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        match key.code {
            KeyCode::PageUp => {
                self.scroll_page_up();
                return Vec::new();
            }
            KeyCode::PageDown => {
                self.scroll_page_down();
                return Vec::new();
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_up();
                return Vec::new();
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_down();
                return Vec::new();
            }
            _ => {}
        }

        if self.streaming() {
            return Vec::new();
        }

        match key.code {
            KeyCode::Enter => self.submit_input(),
            _ => {
                self.input.handle_key(key);
                Vec::new()
            }
        }
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.streaming() {
            return Vec::new();
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.input.insert_str(&pasted);
        Vec::new()
    }

    fn working_history(&self) -> Vec<Message> {
        let skip = self.api_history.len().saturating_sub(self.max_history);
        self.api_history[skip..].to_vec()
    }

    fn submit_input(&mut self) -> Vec<Effect> {
        let input = self.input.text();
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let content = trimmed.to_string();
        self.input = new_editor();
        self.last_error = None;
        self.stream = StreamState::Streaming(String::new());
        self.autoscroll = true;
        self.transcript.push(VisibleMessage {
            speaker: Speaker::User,
            content: content.clone(),
        });
        let history = self.working_history();
        self.push_api_history(Message::user(content.clone()));

        let message_id = uuid::Uuid::new_v4().to_string();
        vec![
            Effect::Persist {
                meeting_id: self.meeting_id.clone(),
                message_id,
                role: Role::User,
                content: content.clone(),
            },
            Effect::Submit {
                prompt: content,
                history,
            },
        ]
    }

    fn finish_stream(&mut self, final_text: Option<String>) -> Vec<Effect> {
        let mut buffer = match std::mem::replace(&mut self.stream, StreamState::Idle) {
            StreamState::Streaming(buf) => buf,
            StreamState::Idle => String::new(),
        };

        if buffer.is_empty()
            && let Some(final_text) = final_text.as_deref()
            && !final_text.is_empty()
        {
            buffer = final_text.to_string();
        } else if let Some(final_text) = final_text.as_deref()
            && final_text.starts_with(&buffer)
            && final_text.len() > buffer.len()
        {
            buffer.push_str(&final_text[buffer.len()..]);
        }

        if buffer.is_empty() {
            self.last_error = Some("Empty response from model".to_string());
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Error,
                content: "No response content received from the model.".to_string(),
            });
            return Vec::new();
        }

        self.transcript.push(VisibleMessage {
            speaker: Speaker::Assistant,
            content: buffer.clone(),
        });

        let message_id = uuid::Uuid::new_v4().to_string();
        let mut effects = vec![Effect::Persist {
            meeting_id: self.meeting_id.clone(),
            message_id,
            role: Role::Assistant,
            content: buffer.clone(),
        }];
        if !self.title_requested {
            self.title_requested = true;
            if let Some(user_msg) = self.transcript.iter().find(|m| m.speaker == Speaker::User) {
                effects.push(Effect::GenerateTitle {
                    prompt: user_msg.content.clone(),
                    response: buffer.clone(),
                });
            }
        }

        self.push_api_history(Message::assistant(buffer));
        effects
    }

    fn fail_stream(&mut self, error: String) -> Vec<Effect> {
        let buffer = match std::mem::replace(&mut self.stream, StreamState::Idle) {
            StreamState::Streaming(buf) => buf,
            StreamState::Idle => String::new(),
        };
        let mut effects = Vec::new();
        if !buffer.is_empty() {
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Assistant,
                content: buffer.clone(),
            });
            let message_id = uuid::Uuid::new_v4().to_string();
            self.push_api_history(Message::assistant(buffer.clone()));
            effects.push(Effect::Persist {
                meeting_id: self.meeting_id.clone(),
                message_id,
                role: Role::Assistant,
                content: buffer,
            });
        }
        self.last_error = Some(error.clone());
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Error,
            content: error,
        });
        effects
    }

    fn push_api_history(&mut self, message: Message) {
        self.api_history.push(message);
        if self.api_history.len() > self.max_history {
            let excess = self.api_history.len() - self.max_history;
            self.api_history.drain(..excess);
        }
    }

    fn scroll_up(&mut self) {
        self.scroll.scroll_up();
        self.autoscroll = false;
    }

    fn scroll_down(&mut self) {
        self.scroll.scroll_down();
    }

    fn scroll_page_up(&mut self) {
        self.scroll.scroll_page_up();
        self.autoscroll = false;
    }

    fn scroll_page_down(&mut self) {
        self.scroll.scroll_page_down();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new("model".to_string(), None, "test-session".to_string())
    }

    #[test]
    fn submit_creates_request_effect() {
        let mut app = test_app();
        app.input_mut().insert_str("hello");

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(effects.iter().any(|e| matches!(e, Effect::Submit { .. })));
        assert!(effects.iter().any(|e| matches!(e, Effect::Persist { .. })));
        assert!(app.streaming());
        assert_eq!(app.transcript.len(), 1);
    }

    #[test]
    fn empty_submit_is_ignored() {
        let mut app = test_app();

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(effects.is_empty());
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn stream_failure_preserves_partial_response() {
        let mut app = test_app();
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamChunk("partial".to_string()));
        let effects = app.dispatch(Action::StreamFailed("boom".to_string()));

        assert_eq!(app.transcript.len(), 3);
        assert_eq!(app.transcript[1].content, "partial");
        assert_eq!(app.transcript[2].speaker, Speaker::Error);
        assert!(effects.iter().any(|e| matches!(e, Effect::Persist { .. })));
    }

    #[test]
    fn api_history_is_capped() {
        let mut app = test_app();
        for idx in 0..(MAX_HISTORY + 5) {
            app.push_api_history(Message::user(format!("message-{idx}")));
        }

        assert_eq!(app.api_history.len(), MAX_HISTORY);
        assert_eq!(app.working_history().len(), MAX_HISTORY);
    }

    #[test]
    fn empty_stream_completion_shows_error() {
        let mut app = test_app();
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamCompleted(None));

        assert!(!app.streaming());
        assert_eq!(app.transcript.len(), 2);
        assert_eq!(app.transcript[0].speaker, Speaker::User);
        assert_eq!(app.transcript[1].speaker, Speaker::Error);
        assert!(app.last_error.is_some());
    }
}
