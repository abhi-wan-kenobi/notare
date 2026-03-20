use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;

const ANIMATION_FRAME: std::time::Duration = std::time::Duration::from_millis(33);
const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) struct Output {
    pub(super) elapsed: std::time::Duration,
    pub(super) force_quit: bool,
    pub(super) app: App,
}

pub(super) enum ExternalEvent {
    Listener(super::runtime::RuntimeEvent),
}

pub(super) struct LiveScreen {
    app: App,
    capture_post_exit_events: Arc<AtomicBool>,
    inspector: crate::interaction_debug::Inspector,
}

impl LiveScreen {
    pub(super) fn new(
        participant_names: HashMap<String, String>,
        capture_post_exit_events: Arc<AtomicBool>,
    ) -> Self {
        Self {
            app: App::new(participant_names),
            capture_post_exit_events,
            inspector: crate::interaction_debug::Inspector::new("meeting-live"),
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Output> {
        for effect in effects {
            match effect {
                Effect::Exit { force } => {
                    crate::tui_trace::trace_effect(
                        "meeting-live",
                        if force { "Exit(force)" } else { "Exit" },
                    );
                    if !force {
                        self.capture_post_exit_events.store(true, Ordering::SeqCst);
                    }
                    let app = std::mem::replace(&mut self.app, App::new(HashMap::new()));
                    return ScreenControl::Exit(Output {
                        elapsed: app.elapsed(),
                        force_quit: force,
                        app,
                    });
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for LiveScreen {
    type ExternalEvent = ExternalEvent;
    type Output = Output;

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
                crate::tui_trace::trace_input_key("meeting-live", &key);
                crate::tui_trace::trace_action("meeting-live", "Key");
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                crate::tui_trace::trace_input_paste("meeting-live", pasted.chars().count());
                crate::tui_trace::trace_action("meeting-live", "Paste");
                let effects = self.app.dispatch(Action::Paste(pasted));
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
        let action = match event {
            ExternalEvent::Listener(event) => {
                crate::tui_trace::trace_external("meeting-live", "Listener");
                crate::tui_trace::trace_action("meeting-live", "RuntimeEvent");
                Action::RuntimeEvent(event)
            }
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &mut self.app);
        self.inspector.draw(frame);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some(&format!(
            "{} ({})",
            self.app.status(),
            crate::output::format_hhmmss(self.app.elapsed())
        )))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.has_active_animations() {
            ANIMATION_FRAME
        } else {
            IDLE_FRAME
        }
    }
}
