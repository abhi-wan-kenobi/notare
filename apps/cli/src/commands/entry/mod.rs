use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod app;
mod ui;

pub use app::EntryAction;

use app::EntryApp;

pub struct Args {
    pub status_message: Option<String>,
    pub initial_command: Option<String>,
}

struct EntryScreen {
    app: EntryApp,
}

impl Screen for EntryScreen {
    type ExternalEvent = Infallible;
    type Output = EntryAction;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => self.app.handle_key(key),
            TuiEvent::Paste(pasted) => self.app.handle_paste(pasted),
            TuiEvent::Draw => {}
        }

        if self.app.should_quit {
            ScreenControl::Exit(self.app.action().unwrap_or(EntryAction::Quit))
        } else {
            ScreenControl::Continue
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
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        "char".into()
    }
}

pub async fn run(args: Args) -> EntryAction {
    let screen = EntryScreen {
        app: EntryApp::new(args.status_message, args.initial_command),
    };

    run_screen::<EntryScreen>(screen, None)
        .await
        .unwrap_or(EntryAction::Quit)
}
