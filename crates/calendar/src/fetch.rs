use hypr_calendar_interface::EventFilter;
use hypr_google_calendar::{
    CalendarListEntry as GoogleCalendar, Event as GoogleEvent, GoogleCalendarClient,
};
use hypr_outlook_calendar::{Calendar as OutlookCalendar, Event as OutlookEvent};

use crate::error::Error;

pub const GOOGLE_API_BASE: &str = "https://www.googleapis.com";
pub const MS_GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";

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

/// List the user's Outlook calendars directly via Microsoft Graph (`/me/calendars`).
pub async fn list_outlook_calendars_direct(
    base_url: &str,
    access_token: &str,
) -> Result<Vec<OutlookCalendar>, Error> {
    let client = reqwest::Client::new();
    let mut calendars = Vec::new();
    let mut next_url = Some(format!("{}/me/calendars", base_url.trim_end_matches('/')));

    while let Some(url) = next_url {
        let response = client
            .get(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(Error::Api(format!(
                "outlook api returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        let page: hypr_outlook_calendar::ListCalendarsResponse = serde_json::from_slice(&bytes)?;
        calendars.extend(page.value);
        next_url = page.odata_next_link.filter(|s| !s.is_empty());
    }

    Ok(calendars)
}

/// List events of one Outlook calendar directly via Microsoft Graph
/// (`/me/calendars/{id}/calendarView?startDateTime=...&endDateTime=...`).
///
/// Handles `@odata.nextLink` paging across multiple pages.
pub async fn list_outlook_events_direct(
    base_url: &str,
    access_token: &str,
    filter: EventFilter,
) -> Result<Vec<OutlookEvent>, Error> {
    let client = reqwest::Client::new();
    let mut events = Vec::new();
    let calendar_id = &filter.calendar_tracking_id;
    let start_iso = filter.from.to_rfc3339();
    let end_iso = filter.to.to_rfc3339();

    let initial_url = format!(
        "{}/me/calendars/{}/calendarView?startDateTime={}&endDateTime={}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(calendar_id),
        urlencoding::encode(&start_iso),
        urlencoding::encode(&end_iso)
    );

    let mut next_url = Some(initial_url);

    while let Some(url) = next_url {
        let response = client
            .get(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(Error::Api(format!(
                "outlook api returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        let page: hypr_outlook_calendar::ListEventsResponse = serde_json::from_slice(&bytes)?;
        events.extend(page.value);
        next_url = page.odata_next_link.filter(|s| !s.is_empty());
    }

    Ok(events)
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

    #[tokio::test]
    async fn lists_outlook_calendars_directly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/calendars"))
            .and(header("authorization", "Bearer ms-at-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "cal-primary",
                        "name": "Calendar",
                        "isDefaultCalendar": true,
                        "canEdit": true,
                        "hexColor": "#0078d4"
                    },
                    {
                        "id": "cal-work",
                        "name": "Work",
                        "isDefaultCalendar": false,
                        "canEdit": true,
                        "hexColor": "#ffaa00"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let calendars = list_outlook_calendars_direct(&server.uri(), "ms-at-1")
            .await
            .unwrap();

        assert_eq!(calendars.len(), 2);
        assert_eq!(calendars[0].id, "cal-primary");
        assert_eq!(calendars[0].name.as_deref(), Some("Calendar"));
        assert_eq!(calendars[0].is_default_calendar, Some(true));
        assert_eq!(calendars[1].id, "cal-work");
    }

    #[tokio::test]
    async fn lists_outlook_events_with_next_link_paging() {
        let server = MockServer::start().await;
        let next_page_path = "/page-2";
        let next_page_url = format!("{}{}", server.uri(), next_page_path);

        // First page returns 1 event and @odata.nextLink pointing to second page
        Mock::given(method("GET"))
            .and(path("/me/calendars/cal-123/calendarView"))
            .and(header("authorization", "Bearer ms-at-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "@odata.nextLink": next_page_url,
                "value": [
                    {
                        "id": "evt-1",
                        "subject": "Sprint Planning",
                        "start": {"dateTime": "2026-11-03T10:00:00Z", "timeZone": "UTC"},
                        "end": {"dateTime": "2026-11-03T11:00:00Z", "timeZone": "UTC"},
                        "iCalUId": "evt-1@outlook.com"
                    }
                ]
            })))
            .mount(&server)
            .await;

        // Second page returns 1 event and null nextLink
        Mock::given(method("GET"))
            .and(path(next_page_path))
            .and(header("authorization", "Bearer ms-at-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "@odata.nextLink": null,
                "value": [
                    {
                        "id": "evt-2",
                        "subject": "Design Review",
                        "start": {"dateTime": "2026-11-03T14:00:00Z", "timeZone": "UTC"},
                        "end": {"dateTime": "2026-11-03T15:00:00Z", "timeZone": "UTC"},
                        "iCalUId": "evt-2@outlook.com"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let filter = EventFilter {
            from: chrono::Utc::now() - chrono::Duration::days(1),
            to: chrono::Utc::now() + chrono::Duration::days(1),
            calendar_tracking_id: "cal-123".to_string(),
        };

        let events = list_outlook_events_direct(&server.uri(), "ms-at-1", filter)
            .await
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, "evt-1");
        assert_eq!(events[0].subject.as_deref(), Some("Sprint Planning"));
        assert_eq!(events[1].id, "evt-2");
        assert_eq!(events[1].subject.as_deref(), Some("Design Review"));
    }

    #[tokio::test]
    async fn surfaces_outlook_api_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/calendars"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": {"code": "InvalidAuthenticationToken", "message": "Access token has expired."}
            })))
            .mount(&server)
            .await;

        let err = list_outlook_calendars_direct(&server.uri(), "expired-token")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("401"));
    }
}
