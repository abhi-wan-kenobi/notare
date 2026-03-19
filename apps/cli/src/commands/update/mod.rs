use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod action;
mod app;
mod effect;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;

pub enum UpdateAction {
    RunUpdate { npm_tag: &'static str },
    Continue,
}

struct UpdateScreen {
    app: App,
}

impl UpdateScreen {
    fn apply_effects(&self, effects: Vec<Effect>) -> ScreenControl<UpdateAction> {
        for effect in effects {
            match effect {
                Effect::AcceptUpdate => {
                    return ScreenControl::Exit(UpdateAction::RunUpdate {
                        npm_tag: self.app.npm_tag,
                    });
                }
                Effect::Skip => return ScreenControl::Exit(UpdateAction::Continue),
                Effect::SkipVersion => {
                    crate::update_check::save_skipped_version(&self.app.latest);
                    return ScreenControl::Exit(UpdateAction::Continue);
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for UpdateScreen {
    type ExternalEvent = Infallible;
    type Output = UpdateAction;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
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
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Update"))
    }
}

pub async fn run(
    current: String,
    latest: String,
    channel: crate::update_check::Channel,
) -> UpdateAction {
    let screen = UpdateScreen {
        app: App::new(current, latest, channel.npm_tag()),
    };

    run_screen::<UpdateScreen>(screen, None)
        .await
        .unwrap_or(UpdateAction::Continue)
}
