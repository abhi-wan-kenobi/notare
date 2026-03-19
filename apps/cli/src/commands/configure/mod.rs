use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::CliResult;

mod action;
mod app;
mod effect;
mod runtime;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::Runtime;

struct ConfigureScreen {
    app: App,
    runtime: Runtime,
}

impl ConfigureScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Exit => return ScreenControl::Exit(()),
                Effect::LoadSettings => self.runtime.load_settings(),
                Effect::SaveSttProvider(p) => self.runtime.save_stt_provider(p),
                Effect::SaveLlmProvider(p) => self.runtime.save_llm_provider(p),
                Effect::LoadCalendars => self.runtime.load_calendars(),
                Effect::SaveCalendars(cals) => self.runtime.save_calendars(cals),
                Effect::CheckCalendarPermission => self.runtime.check_permission(),
                Effect::RequestCalendarPermission => self.runtime.request_permission(),
                Effect::ResetCalendarPermission => self.runtime.reset_permission(),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ConfigureScreen {
    type ExternalEvent = runtime::RuntimeEvent;
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
            _ => ScreenControl::Continue,
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
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Configure"))
    }
}

pub async fn run(pool: &SqlitePool, cli_tab: Option<crate::cli::ConfigureTab>) -> CliResult<()> {
    let initial_tab = cli_tab.map(|t| match t {
        crate::cli::ConfigureTab::Stt => app::Tab::Stt,
        crate::cli::ConfigureTab::Llm => app::Tab::Llm,
        crate::cli::ConfigureTab::Calendar => app::Tab::Calendar,
    });

    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::new(pool.clone(), tx);

    let (app, initial_effects) = App::new(initial_tab);
    let mut screen = ConfigureScreen { app, runtime };

    screen.apply_effects(initial_effects);

    run_screen(screen, Some(rx)).await.map_err(|e| {
        crate::error::CliError::operation_failed("run configure screen", e.to_string())
    })
}
