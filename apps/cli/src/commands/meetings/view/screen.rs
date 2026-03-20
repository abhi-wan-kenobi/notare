use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;
use super::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) struct ViewScreen {
    app: App,
    runtime: Runtime,
    inspector: crate::interaction_debug::Inspector,
}

impl ViewScreen {
    pub(super) fn new(app: App, runtime: Runtime) -> Self {
        Self {
            app,
            runtime,
            inspector: crate::interaction_debug::Inspector::new("meeting-view"),
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::SaveMemo { meeting_id, memo } => {
                    crate::tui_trace::trace_effect("meeting-view", "SaveMemo");
                    self.runtime.save_memo(meeting_id, memo);
                }
                Effect::Exit => {
                    crate::tui_trace::trace_effect("meeting-view", "Exit");
                    return ScreenControl::Exit(());
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ViewScreen {
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
                crate::tui_trace::trace_input_key("meeting-view", &key);
                crate::tui_trace::trace_action("meeting-view", "Key");
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                crate::tui_trace::trace_input_paste("meeting-view", pasted.chars().count());
                crate::tui_trace::trace_action("meeting-view", "Paste");
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
            RuntimeEvent::Loaded {
                meeting,
                segments,
                memo,
            } => {
                crate::tui_trace::trace_external("meeting-view", "Loaded");
                crate::tui_trace::trace_action("meeting-view", "Loaded");
                Action::Loaded {
                    meeting,
                    segments,
                    memo,
                }
            }
            RuntimeEvent::LoadError(msg) => {
                crate::tui_trace::trace_external("meeting-view", "LoadError");
                crate::tui_trace::trace_action("meeting-view", "LoadError");
                Action::LoadError(msg)
            }
            RuntimeEvent::Saved => {
                crate::tui_trace::trace_external("meeting-view", "Saved");
                crate::tui_trace::trace_action("meeting-view", "Saved");
                Action::Saved
            }
            RuntimeEvent::SaveError(msg) => {
                crate::tui_trace::trace_external("meeting-view", "SaveError");
                crate::tui_trace::trace_action("meeting-view", "SaveError");
                Action::SaveError(msg)
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
        hypr_cli_tui::terminal_title(Some("view"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}
