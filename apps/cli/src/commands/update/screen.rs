use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::UpdateOutcome;
use super::app::App;

pub(super) struct UpdateScreen {
    app: App,
    inspector: crate::interaction_debug::Inspector,
}

impl UpdateScreen {
    pub(super) fn new(app: App) -> Self {
        Self {
            app,
            inspector: crate::interaction_debug::Inspector::new("update"),
        }
    }

    fn apply_outcome(&self, outcome: super::app::Outcome) -> ScreenControl<UpdateOutcome> {
        match outcome {
            super::app::Outcome::Continue => ScreenControl::Continue,
            super::app::Outcome::AcceptUpdate => {
                crate::tui_trace::trace_effect("update", "AcceptUpdate");
                ScreenControl::Exit(UpdateOutcome::RunUpdate)
            }
            super::app::Outcome::Skip => {
                crate::tui_trace::trace_effect("update", "Skip");
                ScreenControl::Exit(UpdateOutcome::Continue)
            }
            super::app::Outcome::SkipVersion => {
                crate::tui_trace::trace_effect("update", "SkipVersion");
                crate::update_check::save_skipped_version(&self.app.latest);
                ScreenControl::Exit(UpdateOutcome::Continue)
            }
        }
    }
}

impl Screen for UpdateScreen {
    type ExternalEvent = Infallible;
    type Output = UpdateOutcome;

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
                crate::tui_trace::trace_input_key("update", &key);
                crate::tui_trace::trace_action("update", "Key");
                let outcome = self.app.handle_key(key);
                self.apply_outcome(outcome)
            }
            _ => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {}
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Update"))
    }
}
