use graph_rs_sdk::GraphClient;

use hypr_calendar_interface::{Calendar, CalendarSource, Error, Event, EventFilter};

pub struct Handle {
    client: GraphClient,
}

impl Handle {
    pub async fn new(token: impl Into<String>) -> Self {
        let client = GraphClient::new(token.into());
        Self { client }
    }
}

impl CalendarSource for Handle {
    async fn list_calendars(&self) -> Result<Vec<Calendar>, Error> {
        Ok(vec![])
    }

    async fn list_events(&self, _filter: EventFilter) -> Result<Vec<Event>, Error> {
        Ok(vec![])
    }
}
