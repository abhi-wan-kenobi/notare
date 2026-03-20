use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use super::EntryAction;
use super::action::Action;
use super::app::App;
use super::effect::Effect;

pub(super) enum ExternalEvent {
    ConnectRuntime(crate::commands::connect::runtime::RuntimeEvent),
    MeetingsLoaded(Vec<hypr_db_app::MeetingRow>),
    MeetingsLoadError(String),
    EventsLoaded(Vec<hypr_db_app::EventRow>),
    CalendarNotConfigured,
    ModelsLoaded(Vec<crate::commands::model::list::ModelRow>),
    ModelsLoadError(String),
    ConnectSaved {
        connection_types: Vec<crate::commands::connect::ConnectionType>,
        provider_id: String,
    },
    ConnectSaveError(String),
    TimelineContactsLoaded {
        orgs: Vec<hypr_db_app::OrganizationRow>,
        humans: Vec<hypr_db_app::HumanRow>,
    },
    TimelineContactsLoadError(String),
    TimelineEntriesLoaded(Vec<hypr_db_app::TimelineRow>),
    TimelineEntriesLoadError(String),
}

pub(super) struct EntryScreen {
    app: App,
    external_tx: mpsc::UnboundedSender<ExternalEvent>,
    connect_runtime: crate::commands::connect::runtime::Runtime,
    pool: SqlitePool,
    inspector: crate::interaction_debug::Inspector,
}

impl EntryScreen {
    pub(super) fn new(
        app: App,
        external_tx: mpsc::UnboundedSender<ExternalEvent>,
        connect_runtime: crate::commands::connect::runtime::Runtime,
        pool: SqlitePool,
    ) -> Self {
        Self {
            app,
            external_tx,
            connect_runtime,
            pool,
            inspector: crate::interaction_debug::Inspector::new("entry"),
        }
    }

    pub(super) fn submit_initial_command(&mut self, command: String) -> Option<EntryAction> {
        let effects = self.app.dispatch(Action::SubmitCommand(command));
        if let ScreenControl::Exit(action) = self.apply_effects(effects) {
            Some(action)
        } else {
            None
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<EntryAction> {
        for effect in effects {
            match effect {
                Effect::Launch(cmd) => {
                    crate::tui_trace::trace_effect("entry", "Launch");
                    return ScreenControl::Exit(EntryAction::Launch(cmd));
                }
                Effect::LoadMeetings => {
                    crate::tui_trace::trace_effect("entry", "LoadMeetings");
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::list_meetings(&pool).await {
                            Ok(meetings) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoaded(meetings));
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoadError(e.to_string()));
                            }
                        }
                    });
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::has_calendars(&pool).await {
                            Ok(true) => {
                                let today = chrono::Local::now().date_naive();
                                let start = today.format("%Y-%m-%d").to_string();
                                let end = (today + chrono::Duration::days(2))
                                    .format("%Y-%m-%d")
                                    .to_string();
                                match hypr_db_app::list_events_in_range(&pool, &start, &end).await {
                                    Ok(events) => {
                                        let _ = tx.send(ExternalEvent::EventsLoaded(events));
                                    }
                                    Err(e) => {
                                        let _ = tx
                                            .send(ExternalEvent::MeetingsLoadError(e.to_string()));
                                    }
                                }
                            }
                            Ok(false) => {
                                let _ = tx.send(ExternalEvent::CalendarNotConfigured);
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::LoadModels => {
                    crate::tui_trace::trace_effect("entry", "LoadModels");
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        let paths = crate::config::paths::resolve_paths();
                        let models_base = paths.models_base.clone();
                        let runtime =
                            std::sync::Arc::new(crate::commands::model::runtime::CliModelRuntime {
                                models_base: models_base.clone(),
                                progress_tx: None,
                            });
                        let manager = hypr_model_downloader::ModelDownloadManager::new(runtime);
                        let current = crate::config::paths::load_settings_from_db(&pool).await;
                        let models: Vec<hypr_local_model::LocalModel> =
                            hypr_local_model::LocalModel::all()
                                .into_iter()
                                .filter(|m| crate::commands::model::model_is_enabled(m))
                                .collect();
                        let rows = crate::commands::model::list::collect_model_rows(
                            &models,
                            &models_base,
                            &current,
                            &manager,
                        )
                        .await;
                        let _ = tx.send(ExternalEvent::ModelsLoaded(rows));
                    });
                }
                Effect::LoadTimelineContacts => {
                    crate::tui_trace::trace_effect("entry", "LoadTimelineContacts");
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        let orgs = hypr_db_app::list_organizations(&pool).await;
                        let humans = hypr_db_app::list_humans(&pool).await;
                        match (orgs, humans) {
                            (Ok(orgs), Ok(humans)) => {
                                let _ =
                                    tx.send(ExternalEvent::TimelineContactsLoaded { orgs, humans });
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                let _ = tx
                                    .send(ExternalEvent::TimelineContactsLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::LoadTimelineEntries(human_id) => {
                    crate::tui_trace::trace_effect("entry", "LoadTimelineEntries");
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::list_timeline_by_human(&pool, &human_id).await {
                            Ok(entries) => {
                                let _ = tx.send(ExternalEvent::TimelineEntriesLoaded(entries));
                            }
                            Err(e) => {
                                let _ =
                                    tx.send(ExternalEvent::TimelineEntriesLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::SaveConnect {
                    connection_types,
                    provider,
                    base_url,
                    api_key,
                } => {
                    crate::tui_trace::trace_effect("entry", "SaveConnect");
                    let provider_id = provider.id().to_string();
                    let pool = self.pool.clone();
                    let tx = self.external_tx.clone();
                    let ct = connection_types.clone();
                    tokio::spawn(async move {
                        match crate::commands::connect::save_config(
                            &pool,
                            crate::commands::connect::effect::SaveData {
                                connection_types: ct,
                                provider,
                                base_url,
                                api_key,
                            },
                        )
                        .await
                        {
                            Ok(()) => {
                                let _ = tx.send(ExternalEvent::ConnectSaved {
                                    connection_types,
                                    provider_id,
                                });
                            }
                            Err(error) => {
                                let _ = tx.send(ExternalEvent::ConnectSaveError(error.to_string()));
                            }
                        }
                    });
                }
                Effect::CheckCalendarPermission => {
                    crate::tui_trace::trace_effect("entry", "CheckCalendarPermission");
                    let effects = self.app.dispatch(Action::ConnectRuntime(
                        crate::commands::connect::runtime::RuntimeEvent::CalendarPermissionStatus(
                            crate::commands::connect::runtime::check_permission_sync(),
                        ),
                    ));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::RequestCalendarPermission => {
                    crate::tui_trace::trace_effect("entry", "RequestCalendarPermission");
                    self.connect_runtime.request_permission();
                }
                Effect::ResetCalendarPermission => {
                    crate::tui_trace::trace_effect("entry", "ResetCalendarPermission");
                    self.connect_runtime.reset_permission();
                }
                Effect::LoadCalendars => {
                    crate::tui_trace::trace_effect("entry", "LoadCalendars");
                    let effects = self.app.dispatch(Action::ConnectRuntime(
                        match crate::commands::connect::runtime::load_calendars_sync() {
                            Ok(items) => {
                                crate::commands::connect::runtime::RuntimeEvent::CalendarsLoaded(
                                    items,
                                )
                            }
                            Err(err) => crate::commands::connect::runtime::RuntimeEvent::Error(err),
                        },
                    ));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::SaveCalendars(data) => {
                    crate::tui_trace::trace_effect("entry", "SaveCalendars");
                    let connection_id = format!("cal:{}", data.provider);
                    self.connect_runtime.save_calendars(
                        self.pool.clone(),
                        data.provider,
                        connection_id,
                        data.items,
                    );
                }
                Effect::OpenAuth => {
                    crate::tui_trace::trace_effect("entry", "OpenAuth");
                    let message = match crate::commands::auth::run() {
                        Ok(()) => "Opened auth page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenBug => {
                    crate::tui_trace::trace_effect("entry", "OpenBug");
                    let message = match crate::commands::bug::run() {
                        Ok(()) => "Opened bug report page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenHello => {
                    crate::tui_trace::trace_effect("entry", "OpenHello");
                    let message = match crate::commands::hello::run() {
                        Ok(()) => "Opened char.com in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenDesktop => {
                    crate::tui_trace::trace_effect("entry", "OpenDesktop");
                    let message = match crate::commands::desktop::run() {
                        Ok(crate::commands::desktop::DesktopAction::OpenedApp) => {
                            "Opened desktop app".to_string()
                        }
                        Ok(crate::commands::desktop::DesktopAction::OpenedDownloadPage) => {
                            "Desktop app not found. Opened download page".to_string()
                        }
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::RunModel(cmd) => {
                    crate::tui_trace::trace_effect("entry", "RunModel");
                    return ScreenControl::Exit(EntryAction::Model(cmd));
                }
                Effect::Exit => {
                    crate::tui_trace::trace_effect("entry", "Exit");
                    return ScreenControl::Exit(EntryAction::Quit);
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for EntryScreen {
    type ExternalEvent = ExternalEvent;
    type Output = EntryAction;

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
                crate::tui_trace::trace_input_key("entry", &key);
                crate::tui_trace::trace_action("entry", "Key");
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                crate::tui_trace::trace_input_paste("entry", pasted.chars().count());
                crate::tui_trace::trace_action("entry", "Paste");
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
            ExternalEvent::ConnectRuntime(event) => {
                crate::tui_trace::trace_external("entry", "ConnectRuntime");
                crate::tui_trace::trace_action("entry", "ConnectRuntime");
                Action::ConnectRuntime(event)
            }
            ExternalEvent::MeetingsLoaded(meetings) => {
                crate::tui_trace::trace_external("entry", "MeetingsLoaded");
                crate::tui_trace::trace_action("entry", "MeetingsLoaded");
                Action::MeetingsLoaded(meetings)
            }
            ExternalEvent::MeetingsLoadError(msg) => {
                crate::tui_trace::trace_external("entry", "MeetingsLoadError");
                crate::tui_trace::trace_action("entry", "MeetingsLoadError");
                Action::MeetingsLoadError(msg)
            }
            ExternalEvent::EventsLoaded(events) => {
                crate::tui_trace::trace_external("entry", "EventsLoaded");
                crate::tui_trace::trace_action("entry", "EventsLoaded");
                Action::EventsLoaded(events)
            }
            ExternalEvent::CalendarNotConfigured => {
                crate::tui_trace::trace_external("entry", "CalendarNotConfigured");
                crate::tui_trace::trace_action("entry", "CalendarNotConfigured");
                Action::CalendarNotConfigured
            }
            ExternalEvent::ModelsLoaded(models) => {
                crate::tui_trace::trace_external("entry", "ModelsLoaded");
                crate::tui_trace::trace_action("entry", "ModelsLoaded");
                Action::ModelsLoaded(models)
            }
            ExternalEvent::ModelsLoadError(msg) => {
                crate::tui_trace::trace_external("entry", "ModelsLoadError");
                crate::tui_trace::trace_action("entry", "ModelsLoadError");
                Action::ModelsLoadError(msg)
            }
            ExternalEvent::ConnectSaved {
                connection_types,
                provider_id,
            } => {
                crate::tui_trace::trace_external("entry", "ConnectSaved");
                crate::tui_trace::trace_action("entry", "ConnectSaved");
                Action::ConnectSaved {
                    connection_types,
                    provider_id,
                }
            }
            ExternalEvent::ConnectSaveError(msg) => {
                crate::tui_trace::trace_external("entry", "ConnectSaveError");
                crate::tui_trace::trace_action("entry", "StatusMessage");
                Action::StatusMessage(msg)
            }
            ExternalEvent::TimelineContactsLoaded { orgs, humans } => {
                crate::tui_trace::trace_external("entry", "TimelineContactsLoaded");
                crate::tui_trace::trace_action("entry", "TimelineContactsLoaded");
                Action::TimelineContactsLoaded { orgs, humans }
            }
            ExternalEvent::TimelineContactsLoadError(msg) => {
                crate::tui_trace::trace_external("entry", "TimelineContactsLoadError");
                crate::tui_trace::trace_action("entry", "TimelineContactsLoadError");
                Action::TimelineContactsLoadError(msg)
            }
            ExternalEvent::TimelineEntriesLoaded(entries) => {
                crate::tui_trace::trace_external("entry", "TimelineEntriesLoaded");
                crate::tui_trace::trace_action("entry", "TimelineEntriesLoaded");
                Action::TimelineEntriesLoaded(entries)
            }
            ExternalEvent::TimelineEntriesLoadError(msg) => {
                crate::tui_trace::trace_external("entry", "TimelineEntriesLoadError");
                crate::tui_trace::trace_action("entry", "TimelineEntriesLoadError");
                Action::TimelineEntriesLoadError(msg)
            }
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        super::ui::draw(frame, &mut self.app);
        self.inspector.draw(frame);
    }

    fn on_resize(&mut self) {
        self.app.reload_logo();
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(None)
    }
}
