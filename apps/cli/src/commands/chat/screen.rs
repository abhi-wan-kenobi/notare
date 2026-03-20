use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::action::Action;
use super::app::App;
use super::effect::Effect;
use super::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

pub(super) struct ChatScreen {
    app: App,
    runtime: Runtime,
    inspector: crate::interaction_debug::Inspector,
}

impl ChatScreen {
    pub(super) fn new(app: App, runtime: Runtime) -> Self {
        Self {
            app,
            runtime,
            inspector: crate::interaction_debug::Inspector::new("chat"),
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Submit { prompt, history } => {
                    crate::tui_trace::trace_effect("chat", "Submit");
                    self.runtime.submit(prompt, history);
                }
                Effect::GenerateTitle { prompt, response } => {
                    crate::tui_trace::trace_effect("chat", "GenerateTitle");
                    self.runtime.generate_title(prompt, response);
                }
                Effect::Persist {
                    meeting_id,
                    message_id,
                    role,
                    content,
                } => {
                    crate::tui_trace::trace_effect("chat", "Persist");
                    self.runtime
                        .persist_message(meeting_id, message_id, role, content);
                }
                Effect::UpdateTitle { meeting_id, title } => {
                    crate::tui_trace::trace_effect("chat", "UpdateTitle");
                    self.runtime.update_title(meeting_id, title);
                }
                Effect::Exit => {
                    crate::tui_trace::trace_effect("chat", "Exit");
                    return ScreenControl::Exit(());
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for ChatScreen {
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
                crate::tui_trace::trace_input_key("chat", &key);
                crate::tui_trace::trace_action("chat", "Key");
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                crate::tui_trace::trace_input_paste("chat", pasted.chars().count());
                crate::tui_trace::trace_action("chat", "Paste");
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
            RuntimeEvent::Chunk(chunk) => {
                crate::tui_trace::trace_external("chat", "Chunk");
                crate::tui_trace::trace_action("chat", "StreamChunk");
                Action::StreamChunk(chunk)
            }
            RuntimeEvent::ToolCallStarted {
                tool_name,
                arguments,
            } => {
                crate::tui_trace::trace_external("chat", "ToolCallStarted");
                crate::tui_trace::trace_action("chat", "ToolCallStarted");
                Action::ToolCallStarted {
                    tool_name,
                    arguments,
                }
            }
            RuntimeEvent::Completed(final_text) => {
                crate::tui_trace::trace_external("chat", "Completed");
                crate::tui_trace::trace_action("chat", "StreamCompleted");
                Action::StreamCompleted(final_text)
            }
            RuntimeEvent::Failed(error) => {
                crate::tui_trace::trace_external("chat", "Failed");
                crate::tui_trace::trace_action("chat", "StreamFailed");
                Action::StreamFailed(error)
            }
            RuntimeEvent::TitleGenerated(title) => {
                crate::tui_trace::trace_external("chat", "TitleGenerated");
                crate::tui_trace::trace_action("chat", "TitleGenerated");
                Action::TitleGenerated(title)
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
        self.app.title()
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}
