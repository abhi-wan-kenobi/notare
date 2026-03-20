use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;

pub(super) struct TranscribeScreen {
    app: App,
}

impl TranscribeScreen {
    pub(super) fn new(app: App) -> Self {
        Self { app }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for TranscribeScreen {
    type ExternalEvent = super::runtime::RuntimeEvent;
    type Output = ();

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
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let effects = self.app.dispatch(Action::Runtime(event));
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.is_raw_mode() {
            std::time::Duration::from_millis(50)
        } else if self.app.has_recent_words() {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(100)
        }
    }
}
