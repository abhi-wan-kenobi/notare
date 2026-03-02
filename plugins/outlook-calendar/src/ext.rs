use crate::error::Error;
use crate::fetch;
use crate::types::EventFilter;
use hypr_outlook_calendar::{Calendar, Event};
use tauri_plugin_auth::AuthPluginExt;

pub struct OutlookCalendarExt<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> OutlookCalendarExt<'a, R, M> {
    #[tracing::instrument(skip_all)]
    pub async fn list_calendars(&self) -> Result<Vec<Calendar>, Error> {
        let token = self
            .manager
            .access_token()
            .map_err(|e| Error::Auth(e.to_string()))?;

        match token {
            Some(t) if !t.is_empty() => {
                let config = self.manager.state::<crate::PluginConfig>();
                fetch::list_calendars(&config.api_base_url, &t).await
            }
            _ => Err(Error::NotAuthenticated),
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_events(&self, filter: EventFilter) -> Result<Vec<Event>, Error> {
        let token = self
            .manager
            .access_token()
            .map_err(|e| Error::Auth(e.to_string()))?;

        match token {
            Some(t) if !t.is_empty() => {
                let config = self.manager.state::<crate::PluginConfig>();
                fetch::list_events(&config.api_base_url, &t, filter).await
            }
            _ => Err(Error::NotAuthenticated),
        }
    }
}

pub trait OutlookCalendarPluginExt<R: tauri::Runtime> {
    fn outlook_calendar(&self) -> OutlookCalendarExt<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> OutlookCalendarPluginExt<R> for T {
    fn outlook_calendar(&self) -> OutlookCalendarExt<'_, R, Self>
    where
        Self: Sized,
    {
        OutlookCalendarExt {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
