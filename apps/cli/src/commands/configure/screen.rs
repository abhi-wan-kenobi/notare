use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;
use super::runtime::{Runtime, RuntimeEvent};

pub(super) struct ConfigureScreen {
    app: App,
    runtime: Runtime,
    inspector: crate::interaction_debug::Inspector,
}

impl ConfigureScreen {
    pub(super) fn new(app: App, runtime: Runtime) -> Self {
        Self {
            app,
            runtime,
            inspector: crate::interaction_debug::Inspector::new("configure"),
        }
    }

    pub(super) fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Exit => {
                    crate::tui_trace::trace_effect("configure", "Exit");
                    return ScreenControl::Exit(());
                }
                Effect::LoadSettings => {
                    crate::tui_trace::trace_effect("configure", "LoadSettings");
                    self.runtime.load_settings();
                }
                Effect::SaveProvider { tab, provider } => {
                    crate::tui_trace::trace_effect("configure", "SaveProvider");
                    self.runtime.save_provider(tab, provider);
                }
                Effect::LoadCalendars => {
                    crate::tui_trace::trace_effect("configure", "LoadCalendars");
                    self.runtime.load_calendars();
                }
                Effect::SaveCalendars(cals) => {
                    crate::tui_trace::trace_effect("configure", "SaveCalendars");
                    self.runtime.save_calendars(cals);
                }
                Effect::CheckCalendarPermission => {
                    crate::tui_trace::trace_effect("configure", "CheckCalendarPermission");
                    self.runtime.check_permission();
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ConfigureScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = ();

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
                crate::tui_trace::trace_input_key("configure", &key);
                crate::tui_trace::trace_action("configure", "Key");
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
        crate::tui_trace::trace_external(
            "configure",
            match &event {
                RuntimeEvent::SettingsLoaded { .. } => "SettingsLoaded",
                RuntimeEvent::CalendarsLoaded(_) => "CalendarsLoaded",
                RuntimeEvent::CalendarPermissionStatus(_) => "CalendarPermissionStatus",
                RuntimeEvent::Saved => "Saved",
                RuntimeEvent::Error(_) => "Error",
            },
        );
        crate::tui_trace::trace_action("configure", "Runtime");
        let effects = self.app.dispatch(Action::Runtime(event));
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &mut self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Configure"))
    }
}
