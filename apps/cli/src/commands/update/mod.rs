use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod action;
mod app;
mod effect;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;

pub enum UpdateOutcome {
    RunUpdate,
    Continue,
}

struct UpdateScreen {
    app: App,
    inspector: crate::interaction_debug::Inspector,
}

impl UpdateScreen {
    fn apply_effects(&self, effects: Vec<Effect>) -> ScreenControl<UpdateOutcome> {
        for effect in effects {
            match effect {
                Effect::AcceptUpdate => {
                    crate::tui_trace::trace_effect("update", "AcceptUpdate");
                    return ScreenControl::Exit(UpdateOutcome::RunUpdate);
                }
                Effect::Skip => {
                    crate::tui_trace::trace_effect("update", "Skip");
                    return ScreenControl::Exit(UpdateOutcome::Continue);
                }
                Effect::SkipVersion => {
                    crate::tui_trace::trace_effect("update", "SkipVersion");
                    crate::update_check::save_skipped_version(&self.app.latest);
                    return ScreenControl::Exit(UpdateOutcome::Continue);
                }
            }
        }
        ScreenControl::Continue
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
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
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
        ui::draw(frame, &self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Update"))
    }
}

pub async fn run(
    current: String,
    latest: String,
    action: &crate::update_check::UpdateAction,
) -> UpdateOutcome {
    let screen = UpdateScreen {
        app: App::new(current, latest, action.command_str()),
        inspector: crate::interaction_debug::Inspector::new("update"),
    };

    run_screen::<UpdateScreen>(screen, None)
        .await
        .unwrap_or(UpdateOutcome::Continue)
}
