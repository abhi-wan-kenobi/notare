use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;
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

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<String>> {
        for effect in effects {
            match effect {
                Effect::Select(id) => {
                    crate::tui_trace::trace_effect("meetings", "Select");
                    return ScreenControl::Exit(Some(id));
                }
                Effect::Exit => {
                    crate::tui_trace::trace_effect("meetings", "Exit");
                    return ScreenControl::Exit(None);
                }
            }
        }
        ScreenControl::Continue
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
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            RuntimeEvent::MeetingsLoaded(meetings) => {
                crate::tui_trace::trace_external("meetings", "MeetingsLoaded");
                crate::tui_trace::trace_action("meetings", "MeetingsLoaded");
                Action::MeetingsLoaded(meetings)
            }
            RuntimeEvent::EventsLoaded(events) => {
                crate::tui_trace::trace_external("meetings", "EventsLoaded");
                crate::tui_trace::trace_action("meetings", "EventsLoaded");
                Action::EventsLoaded(events)
            }
            RuntimeEvent::CalendarNotConfigured => {
                crate::tui_trace::trace_external("meetings", "CalendarNotConfigured");
                crate::tui_trace::trace_action("meetings", "CalendarNotConfigured");
                Action::CalendarNotConfigured
            }
            RuntimeEvent::LoadError(msg) => {
                crate::tui_trace::trace_external("meetings", "LoadError");
                crate::tui_trace::trace_action("meetings", "LoadError");
                Action::LoadError(msg)
            }
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
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
