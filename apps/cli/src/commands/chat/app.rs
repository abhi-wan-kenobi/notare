use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use rig::message::Message;

use crate::theme::Theme;
use crate::widgets::ScrollViewState;

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

pub(crate) struct App {
    model: String,
    session: Option<String>,
    db_path: PathBuf,
    session_id: String,
    api_history: Vec<Message>,
    max_history: usize,
    transcript: Vec<VisibleMessage>,
    input: Editor<Theme>,
    pending_assistant: String,
    streaming: bool,
    status: String,
    last_error: Option<String>,
    started_at: Instant,
    scroll: ScrollViewState,
    autoscroll: bool,
    terminal_title: Option<String>,
    title_requested: bool,
}

impl App {
    pub(crate) fn new(
        model: String,
        session: Option<String>,
        db_path: PathBuf,
        session_id: String,
    ) -> Self {
        let mut input = Editor::with_styles(Theme::DEFAULT);
        input.set_placeholder(
            "Type a message and press Enter...",
            Theme::DEFAULT.placeholder,
        );

        let status = if session.is_some() {
            "Ready (session loaded)".to_string()
        } else {
            "Ready".to_string()
        };

        Self {
            model,
            session,
            db_path,
            session_id,
            api_history: Vec::new(),
            max_history: MAX_HISTORY,
            transcript: Vec::new(),
            input,
            pending_assistant: String::new(),
            streaming: false,
            status,
            last_error: None,
            started_at: Instant::now(),
            scroll: ScrollViewState::new(),
            autoscroll: true,
            terminal_title: None,
            title_requested: false,
        }
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
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
                Speaker::User => self.api_history.push(Message::user(msg.content)),
                Speaker::Assistant => self.api_history.push(Message::assistant(msg.content)),
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
                self.pending_assistant.push_str(&chunk);
                self.status = "Streaming response...".to_string();
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
                    db_path: self.db_path.clone(),
                    session_id: self.session_id.clone(),
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
        &self.pending_assistant
    }

    pub(crate) fn streaming(&self) -> bool {
        self.streaming
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut ScrollViewState {
        if self.autoscroll {
            self.scroll.scroll_to_bottom();
        }
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

        if self.streaming {
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
        if self.streaming {
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
        self.input = Editor::with_styles(Theme::DEFAULT);
        self.input.set_placeholder(
            "Type a message and press Enter...",
            Theme::DEFAULT.placeholder,
        );
        self.last_error = None;
        self.streaming = true;
        self.pending_assistant.clear();
        self.autoscroll = true;
        self.status = "Streaming response...".to_string();
        self.transcript.push(VisibleMessage {
            speaker: Speaker::User,
            content: content.clone(),
        });
        let history = self.working_history();
        self.api_history.push(Message::user(content.clone()));

        let message_id = uuid::Uuid::new_v4().to_string();
        vec![
            Effect::Persist {
                db_path: self.db_path.clone(),
                session_id: self.session_id.clone(),
                message_id,
                role: "user".to_string(),
                content: content.clone(),
            },
            Effect::Submit {
                prompt: content,
                history,
            },
        ]
    }

    fn finish_stream(&mut self, final_text: Option<String>) -> Vec<Effect> {
        self.streaming = false;
        self.status = "Ready".to_string();

        if self.pending_assistant.is_empty()
            && let Some(final_text) = final_text.as_deref()
            && !final_text.is_empty()
        {
            self.pending_assistant = final_text.to_string();
        } else if let Some(final_text) = final_text.as_deref()
            && final_text.starts_with(&self.pending_assistant)
            && final_text.len() > self.pending_assistant.len()
        {
            self.pending_assistant
                .push_str(&final_text[self.pending_assistant.len()..]);
        }

        if self.pending_assistant.is_empty() {
            self.last_error = Some("Empty response from model".to_string());
            self.status = "Error: empty response".to_string();
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Error,
                content: "No response content received from the model.".to_string(),
            });
            return Vec::new();
        }

        let content = std::mem::take(&mut self.pending_assistant);
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Assistant,
            content: content.clone(),
        });

        let message_id = uuid::Uuid::new_v4().to_string();
        let mut effects = vec![Effect::Persist {
            db_path: self.db_path.clone(),
            session_id: self.session_id.clone(),
            message_id,
            role: "assistant".to_string(),
            content: content.clone(),
        }];
        if !self.title_requested {
            self.title_requested = true;
            if let Some(user_msg) = self.transcript.iter().find(|m| m.speaker == Speaker::User) {
                effects.push(Effect::GenerateTitle {
                    prompt: user_msg.content.clone(),
                    response: content.clone(),
                });
            }
        }

        self.api_history.push(Message::assistant(content));
        effects
    }

    fn fail_stream(&mut self, error: String) -> Vec<Effect> {
        self.streaming = false;
        if !self.pending_assistant.is_empty() {
            let content = std::mem::take(&mut self.pending_assistant);
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Assistant,
                content: content.clone(),
            });
            self.api_history.push(Message::assistant(content));
        }
        self.last_error = Some(error.clone());
        self.status = format!("Error: {error}");
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Error,
            content: error,
        });
        Vec::new()
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
        App::new(
            "model".to_string(),
            None,
            PathBuf::from("/tmp/test.db"),
            "test-session".to_string(),
        )
    }

    #[test]
    fn submit_creates_request_effect() {
        let mut app = test_app();
        app.input_mut().insert_str("hello");

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(effects.iter().any(|e| matches!(e, Effect::Submit { .. })));
        assert!(effects.iter().any(|e| matches!(e, Effect::Persist { .. })));
        assert!(app.streaming);
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
        let _ = app.dispatch(Action::StreamFailed("boom".to_string()));

        assert_eq!(app.transcript.len(), 3);
        assert_eq!(app.transcript[1].content, "partial");
        assert_eq!(app.transcript[2].speaker, Speaker::Error);
    }

    #[test]
    fn empty_stream_completion_shows_error() {
        let mut app = test_app();
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamCompleted(None));

        assert!(!app.streaming);
        assert_eq!(app.transcript.len(), 2);
        assert_eq!(app.transcript[0].speaker, Speaker::User);
        assert_eq!(app.transcript[1].speaker, Speaker::Error);
        assert!(app.last_error.is_some());
    }
}
