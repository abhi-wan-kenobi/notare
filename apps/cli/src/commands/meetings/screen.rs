use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::app::App;
use super::runtime::RuntimeEvent;

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) struct MeetingsScreen {
    app: App,
    inspector: crate::interaction_debug::Inspector,
}

impl MeetingsScreen {
    pub(super) fn new(app: App) -> Self {
        Self {
            app,
            inspector: crate::interaction_debug::Inspector::new("meetings"),
        }
    }

    fn apply_outcome(&mut self, outcome: super::app::Outcome) -> ScreenControl<Option<String>> {
        match outcome {
            super::app::Outcome::Continue => ScreenControl::Continue,
            super::app::Outcome::Select(id) => {
                crate::tui_trace::trace_effect("meetings", "Select");
                ScreenControl::Exit(Some(id))
            }
            super::app::Outcome::Exit => {
                crate::tui_trace::trace_effect("meetings", "Exit");
                ScreenControl::Exit(None)
            }
        }
    }
}

impl Screen for MeetingsScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = Option<String>;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                if self.inspector.handle_key(key) {
                    return ScreenControl::Continue;
                }
                crate::tui_trace::trace_input_key("meetings", &key);
                crate::tui_trace::trace_action("meetings", "Key");
                let outcome = self.app.handle_key(key);
                self.apply_outcome(outcome)
            }
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            RuntimeEvent::MeetingsLoaded(meetings) => {
                crate::tui_trace::trace_external("meetings", "MeetingsLoaded");
                self.app.set_meetings(meetings);
            }
            RuntimeEvent::EventsLoaded(events) => {
                crate::tui_trace::trace_external("meetings", "EventsLoaded");
                self.app.set_events(events);
            }
            RuntimeEvent::CalendarNotConfigured => {
                crate::tui_trace::trace_external("meetings", "CalendarNotConfigured");
                self.app.set_calendar_not_configured();
            }
            RuntimeEvent::LoadError(msg) => {
                crate::tui_trace::trace_external("meetings", "LoadError");
                self.app.set_error(msg);
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::list::draw(frame, &mut self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("meetings"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}
