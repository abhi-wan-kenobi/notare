use hypr_calendar_interface::EventFilter;
use hypr_google_calendar::{
    CalendarListEntry as GoogleCalendar, Event as GoogleEvent, GoogleCalendarClient,
};
use hypr_outlook_calendar::{Calendar as OutlookCalendar, Event as OutlookEvent};

use crate::error::Error;

pub const GOOGLE_API_BASE: &str = "https://www.googleapis.com";

/// Minimal `hypr_http::HttpClient` that talks straight to the Google REST API
/// with a bearer token — this replaces the dead upstream-cloud proxy route.
struct GoogleRestHttp {
    client: reqwest::Client,
    base_url: String,
}

impl GoogleRestHttp {
    fn new(base_url: &str, access_token: &str) -> Result<Self, Error> {
        let auth_value = format!("Bearer {access_token}").parse()?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<(Vec<u8>, String)>,
    ) -> Result<Vec<u8>, hypr_http::Error> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.request(method, &url);
        if let Some((body, content_type)) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body);
        }
        let response = request.send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(format!(
                "google api returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )
            .into());
        }
        Ok(bytes.to_vec())
    }
}

impl hypr_http::HttpClient for GoogleRestHttp {
    async fn get(&self, path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(reqwest::Method::GET, path, None).await
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::POST,
            path,
            Some((body, content_type.to_string())),
        )
        .await
    }

    async fn put(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::PUT,
            path,
            Some((body, "application/json".to_string())),
        )
        .await
    }

    async fn patch(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::PATCH,
            path,
            Some((body, "application/json".to_string())),
        )
        .await
    }

    async fn delete(&self, path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(reqwest::Method::DELETE, path, None).await
    }
}

/// List the user's Google calendars directly via the Google Calendar REST API
/// (BYO OAuth client; `access_token` is a Google access token).
pub async fn list_google_calendars_direct(
    base_url: &str,
    access_token: &str,
) -> Result<Vec<GoogleCalendar>, Error> {
    let http = GoogleRestHttp::new(base_url, access_token)?;
    let client = GoogleCalendarClient::new(http);
    let mut response = client
        .list_calendars()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;
    // Hidden/deleted entries are noise for sync purposes.
    response
        .items
        .retain(|c| c.deleted != Some(true) && c.hidden != Some(true));
    Ok(response.items)
}

/// List events of one Google calendar directly via the Google Calendar REST API.
pub async fn list_google_events_direct(
    base_url: &str,
    access_token: &str,
    filter: EventFilter,
) -> Result<Vec<GoogleEvent>, Error> {
    let http = GoogleRestHttp::new(base_url, access_token)?;
    let client = GoogleCalendarClient::new(http);

    let request = hypr_google_calendar::ListEventsRequest {
        calendar_id: filter.calendar_tracking_id,
        time_min: Some(filter.from),
        time_max: Some(filter.to),
        single_events: Some(true),
        order_by: Some(hypr_google_calendar::EventOrderBy::StartTime),
        max_results: Some(2500),
        ..Default::default()
    };

    let response = client
        .list_events(request)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;
    Ok(response.items)
}

pub async fn list_all_connection_ids(
    api_base_url: &str,
    access_token: &str,
) -> Result<Vec<(String, Vec<String>)>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let response = client
        .list_connections()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    let connections = response.into_inner().connections;
    let mut map = std::collections::HashMap::<String, Vec<String>>::new();
    for c in &connections {
        map.entry(c.integration_id.clone())
            .or_default()
            .push(c.connection_id.clone());
    }

    Ok(map.into_iter().collect())
}

fn make_client(api_base_url: &str, access_token: &str) -> Result<hypr_api_client::Client, Error> {
    let auth_value = format!("Bearer {access_token}").parse()?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(hypr_api_client::Client::new_with_client(api_base_url, http))
}

pub async fn list_outlook_calendars(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
) -> Result<Vec<OutlookCalendar>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::OutlookListCalendarsRequest {
        connection_id: connection_id.to_string(),
    };

    let response = client
        .outlook_list_calendars(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().value)
}

pub async fn list_outlook_events(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
    filter: EventFilter,
) -> Result<Vec<OutlookEvent>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::OutlookListEventsRequest {
        connection_id: connection_id.to_string(),
        calendar_id: filter.calendar_tracking_id,
        time_min: Some(filter.from.to_rfc3339()),
        time_max: Some(filter.to.to_rfc3339()),
        max_results: None,
        order_by: Some("startTime".to_string()),
    };

    let response = client
        .outlook_list_events(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().value)
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn lists_google_calendars_directly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/calendar/v3/users/me/calendarList"))
            .and(header("authorization", "Bearer at-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "kind": "calendar#calendarList",
                "items": [
                    {"id": "primary@example.com", "summary": "Personal", "primary": true, "accessRole": "owner"},
                    {"id": "team@group.calendar.google.com", "summary": "Team", "accessRole": "reader"},
                    {"id": "gone@group.calendar.google.com", "summary": "Old", "deleted": true},
                    {"id": "hidden@group.calendar.google.com", "summary": "Hidden", "hidden": true}
                ]
            })))
            .mount(&server)
            .await;

        let calendars = list_google_calendars_direct(&server.uri(), "at-1")
            .await
            .unwrap();

        assert_eq!(calendars.len(), 2);
        assert_eq!(calendars[0].id, "primary@example.com");
        assert_eq!(calendars[0].primary, Some(true));
        assert_eq!(calendars[1].id, "team@group.calendar.google.com");
    }

    #[tokio::test]
    async fn lists_google_events_directly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            // '#' and '@' in the calendar id must be percent-encoded into the path.
            .and(path(
                "/calendar/v3/calendars/addressbook%23contacts%40group.v.calendar.google.com/events",
            ))
            .and(header("authorization", "Bearer at-1"))
            .and(query_param("singleEvents", "true"))
            .and(query_param("orderBy", "startTime"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "kind": "calendar#events",
                "items": [
                    {
                        "id": "evt-1",
                        "summary": "Standup",
                        "status": "confirmed",
                        "start": {"dateTime": "2026-07-16T09:00:00+05:30"},
                        "end": {"dateTime": "2026-07-16T09:15:00+05:30"},
                        "iCalUID": "evt-1@google.com"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let filter = EventFilter {
            from: chrono::Utc::now() - chrono::Duration::days(1),
            to: chrono::Utc::now() + chrono::Duration::days(1),
            calendar_tracking_id: "addressbook#contacts@group.v.calendar.google.com".to_string(),
        };
        let events = list_google_events_direct(&server.uri(), "at-1", filter)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "evt-1");
        assert_eq!(events[0].summary.as_deref(), Some("Standup"));
    }

    #[tokio::test]
    async fn surfaces_google_api_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/calendar/v3/users/me/calendarList"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": {"code": 401, "message": "Invalid Credentials"}
            })))
            .mount(&server)
            .await;

        let err = list_google_calendars_direct(&server.uri(), "expired")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("401"));
    }
}
