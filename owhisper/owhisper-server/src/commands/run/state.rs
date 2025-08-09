use std::time::{Duration, Instant};

use super::event::TuiEventSender;
use ratatui::widgets::{ListState, ScrollbarState};

pub struct RunState {
    pub transcripts: Vec<TranscriptEntry>,
    pub start_time: Instant,
    pub scroll_state: ScrollbarState,
    pub scroll_position: usize,
    pub processing: bool,
    pub last_activity: Instant,
    pub current_device: String,
    pub available_devices: Vec<String>,
    pub device_list_state: ListState,
    pub show_device_selector: bool,
    pub event_sender: Option<TuiEventSender>,
}

#[derive(Clone)]
pub struct TranscriptEntry {
    pub text: String,
    pub timestamp: Instant,
}

impl RunState {
    pub fn new(current_device: String, available_devices: Vec<String>) -> Self {
        let mut device_list_state = ListState::default();

        // Select the current device in the list
        if let Some(index) = available_devices.iter().position(|d| d == &current_device) {
            device_list_state.select(Some(index));
        }

        Self {
            transcripts: Vec::new(),
            start_time: Instant::now(),
            scroll_state: ScrollbarState::default(),
            scroll_position: 0,
            processing: false,
            last_activity: Instant::now(),
            current_device,
            available_devices,
            device_list_state,
            show_device_selector: false,
            event_sender: None,
        }
    }

    pub fn set_event_sender(&mut self, sender: TuiEventSender) {
        self.event_sender = Some(sender);
    }

    pub fn process_chunk(&mut self, chunk: owhisper_interface::ListenOutputChunk) {
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

        self.processing = true;
        self.last_activity = Instant::now();

        self.scroll_position = self.transcripts.len().saturating_sub(1);
        self.update_scroll_state();
    }

    pub fn scroll_down(&mut self) {
        self.scroll_position =
            (self.scroll_position + 1).min(self.transcripts.len().saturating_sub(1));
        self.update_scroll_state();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
        self.update_scroll_state();
    }

    pub fn update_scroll_state(&mut self) {
        self.scroll_state = self
            .scroll_state
            .content_length(self.transcripts.len())
            .position(self.scroll_position);
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn is_active(&self) -> bool {
        self.last_activity.elapsed() < Duration::from_secs(2)
    }
}
