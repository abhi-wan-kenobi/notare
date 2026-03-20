use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};
use sqlx::SqlitePool;

use super::action::Action;
use super::app::App;
use super::effect::{Effect, SaveData};
use super::runtime::{self, Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

pub(super) struct ConnectScreen {
    app: App,
    runtime: Runtime,
    pool: SqlitePool,
    inspector: crate::interaction_debug::Inspector,
}

impl ConnectScreen {
    pub(super) fn new(app: App, runtime: Runtime, pool: SqlitePool) -> Self {
        Self {
            app,
            runtime,
            pool,
            inspector: crate::interaction_debug::Inspector::new("connect"),
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<SaveData>> {
        for effect in effects {
            match effect {
                Effect::Save(data) => {
                    crate::tui_trace::trace_effect("connect", "Save");
                    return ScreenControl::Exit(Some(data));
                }
                Effect::Exit => {
                    crate::tui_trace::trace_effect("connect", "Exit");
                    return ScreenControl::Exit(None);
                }
                Effect::CheckCalendarPermission => {
                    crate::tui_trace::trace_effect("connect", "CheckCalendarPermission");
                    self.runtime.check_permission();
                }
                Effect::RequestCalendarPermission => {
                    crate::tui_trace::trace_effect("connect", "RequestCalendarPermission");
                    self.runtime.request_permission();
                }
                Effect::ResetCalendarPermission => {
                    crate::tui_trace::trace_effect("connect", "ResetCalendarPermission");
                    self.runtime.reset_permission();
                }
                Effect::LoadCalendars => {
                    crate::tui_trace::trace_effect("connect", "LoadCalendars");
                    let event = match runtime::load_calendars_sync() {
                        Ok(items) => RuntimeEvent::CalendarsLoaded(items),
                        Err(err) => RuntimeEvent::Error(err),
                    };
                    crate::tui_trace::trace_action(
                        "connect",
                        match &event {
                            RuntimeEvent::CalendarsLoaded(_) => "Runtime::CalendarsLoaded",
                            RuntimeEvent::Error(_) => "Runtime::Error",
                            RuntimeEvent::CalendarPermissionStatus(_) => {
                                "Runtime::CalendarPermissionStatus"
                            }
                            RuntimeEvent::CalendarPermissionResult(_) => {
                                "Runtime::CalendarPermissionResult"
                            }
                            RuntimeEvent::CalendarPermissionReset => {
                                "Runtime::CalendarPermissionReset"
                            }
                            RuntimeEvent::CalendarsSaved => "Runtime::CalendarsSaved",
                        },
                    );
                    let effects = self.app.dispatch(Action::Runtime(event));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::SaveCalendars(data) => {
                    crate::tui_trace::trace_effect("connect", "SaveCalendars");
                    let provider = self.app.provider().unwrap();
                    let connection_id = format!("cal:{}", provider.id());
                    self.runtime.save_calendars(
                        self.pool.clone(),
                        data.provider,
                        connection_id,
                        data.items,
                    );
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ConnectScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = Option<SaveData>;

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
                crate::tui_trace::trace_input_key("connect", &key);
                crate::tui_trace::trace_action("connect", "Key");
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(text) => {
                crate::tui_trace::trace_input_paste("connect", text.chars().count());
                crate::tui_trace::trace_action("connect", "Paste");
                let effects = self.app.dispatch(Action::Paste(text));
                self.apply_effects(effects)
            }
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        crate::tui_trace::trace_external(
            "connect",
            match &event {
                RuntimeEvent::CalendarPermissionStatus(_) => "CalendarPermissionStatus",
                RuntimeEvent::CalendarPermissionResult(_) => "CalendarPermissionResult",
                RuntimeEvent::CalendarPermissionReset => "CalendarPermissionReset",
                RuntimeEvent::CalendarsLoaded(_) => "CalendarsLoaded",
                RuntimeEvent::CalendarsSaved => "CalendarsSaved",
                RuntimeEvent::Error(_) => "Error",
            },
        );
        crate::tui_trace::trace_action("connect", "Runtime");
        let effects = self.app.dispatch(Action::Runtime(event));
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &mut self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("connect"))
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}
