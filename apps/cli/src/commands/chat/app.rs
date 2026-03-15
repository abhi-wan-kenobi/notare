use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_tui::textarea_input_from_key_event;
use hypr_openrouter::{ChatMessage, Role};
use tui_textarea::TextArea;

use crate::fmt::format_hhmmss;
use crate::theme::Theme;

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

pub(crate) struct App {
    model: String,
    session: Option<String>,
    system_message: Option<String>,
    api_history: Vec<ChatMessage>,
    transcript: Vec<VisibleMessage>,
    input: TextArea<'static>,
    pending_assistant: String,
    streaming: bool,
    status: String,
    last_error: Option<String>,
    started_at: Instant,
    scroll_offset: u16,
    max_scroll: u16,
    autoscroll: bool,
}

impl App {
    pub(crate) fn new(
        model: String,
        session: Option<String>,
        system_message: Option<String>,
    ) -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("Type a message and press Enter...");
        input.set_placeholder_style(Theme::default().placeholder);

        let status = if session.is_some() {
            "Ready (session loaded)".to_string()
        } else {
            "Ready".to_string()
        };

        Self {
            model,
            session,
            system_message,
            api_history: Vec::new(),
            transcript: Vec::new(),
            input,
            pending_assistant: String::new(),
            streaming: false,
            status,
            last_error: None,
            started_at: Instant::now(),
            scroll_offset: 0,
            max_scroll: 0,
            autoscroll: true,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::StreamChunk(chunk) => {
                self.pending_assistant.push_str(&chunk);
                self.status = "Streaming response...".to_string();
                if self.autoscroll {
                    self.scroll_offset = self.max_scroll;
                }
                Vec::new()
            }
            Action::StreamCompleted => {
                self.finish_stream();
                Vec::new()
            }
            Action::StreamFailed(error) => {
                self.fail_stream(error);
                Vec::new()
            }
        }
    }

    pub(crate) fn title(&self) -> String {
        let label = if self.streaming {
            "streaming"
        } else {
            self.status.as_str()
        };
        format!("char chat: {label} ({})", format_hhmmss(self.elapsed()))
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub(crate) fn input(&self) -> &TextArea<'static> {
        &self.input
    }

    pub(crate) fn input_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.input
    }

    pub(crate) fn transcript(&self) -> &[VisibleMessage] {
        &self.transcript
    }

    pub(crate) fn pending_assistant(&self) -> &str {
        &self.pending_assistant
    }

    pub(crate) fn streaming(&self) -> bool {
        self.streaming
    }

    pub(crate) fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    pub(crate) fn update_max_scroll(&mut self, max_scroll: u16) {
        self.max_scroll = max_scroll;
        if self.autoscroll {
            self.scroll_offset = max_scroll;
        } else {
            self.scroll_offset = self.scroll_offset.min(max_scroll);
        }
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

        if self.streaming {
            return Vec::new();
        }

        match key.code {
            KeyCode::Enter => self.submit_input(),
            _ => {
                if let Some(input) = textarea_input_from_key_event(key, false) {
                    self.input.input(input);
                }
                Vec::new()
            }
        }
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.streaming {
            return Vec::new();
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.input.insert_str(&pasted);
        Vec::new()
    }

    fn submit_input(&mut self) -> Vec<Effect> {
        let input = self.input.lines().join("\n");
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let content = trimmed.to_string();
        self.input = TextArea::default();
        self.input
            .set_placeholder_text("Type a message and press Enter...");
        self.input
            .set_placeholder_style(Theme::default().placeholder);
        self.last_error = None;
        self.streaming = true;
        self.pending_assistant.clear();
        self.autoscroll = true;
        self.status = "Streaming response...".to_string();
        self.transcript.push(VisibleMessage {
            speaker: Speaker::User,
            content: content.clone(),
        });
        self.api_history
            .push(ChatMessage::new(Role::User, content.clone()));

        let mut messages = Vec::new();
        if let Some(system_message) = self.system_message.as_deref() {
            messages.push(ChatMessage::new(Role::System, system_message));
        }
        messages.extend(self.api_history.iter().cloned());

        vec![Effect::Submit { messages }]
    }

    fn finish_stream(&mut self) {
        self.streaming = false;
        self.status = "Ready".to_string();

        if self.pending_assistant.is_empty() {
            return;
        }

        let content = std::mem::take(&mut self.pending_assistant);
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Assistant,
            content: content.clone(),
        });
        self.api_history
            .push(ChatMessage::new(Role::Assistant, content));
    }

    fn fail_stream(&mut self, error: String) {
        self.streaming = false;
        if !self.pending_assistant.is_empty() {
            let content = std::mem::take(&mut self.pending_assistant);
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Assistant,
                content: content.clone(),
            });
            self.api_history
                .push(ChatMessage::new(Role::Assistant, content));
        }
        self.last_error = Some(error.clone());
        self.status = format!("Error: {error}");
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Error,
            content: error,
        });
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.autoscroll = false;
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1).min(self.max_scroll);
        self.autoscroll = self.scroll_offset >= self.max_scroll;
    }

    fn scroll_page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
        self.autoscroll = false;
    }

    fn scroll_page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(10).min(self.max_scroll);
        self.autoscroll = self.scroll_offset >= self.max_scroll;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_creates_request_effect() {
        let mut app = App::new("model".to_string(), None, None);
        app.input_mut().insert_str("hello");

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(matches!(effects.first(), Some(Effect::Submit { .. })));
        assert!(app.streaming);
        assert_eq!(app.transcript.len(), 1);
    }

    #[test]
    fn empty_submit_is_ignored() {
        let mut app = App::new("model".to_string(), None, None);

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(effects.is_empty());
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn stream_failure_preserves_partial_response() {
        let mut app = App::new("model".to_string(), None, None);
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamChunk("partial".to_string()));
        let _ = app.dispatch(Action::StreamFailed("boom".to_string()));

        assert_eq!(app.transcript.len(), 3);
        assert_eq!(app.transcript[1].content, "partial");
        assert_eq!(app.transcript[2].speaker, Speaker::Error);
    }
}
