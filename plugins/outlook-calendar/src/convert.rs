use hypr_calendar_interface::{
    AttendeeRole, AttendeeStatus, CalendarEvent, CalendarProviderType, EventAttendee, EventPerson,
    EventStatus,
};
use hypr_outlook_calendar::{
    Attendee, AttendeeType, Event, EventShowAs, ResponseType as OutlookResponseType,
};

pub fn convert_events(events: Vec<Event>, calendar_id: &str) -> Vec<CalendarEvent> {
    events
        .into_iter()
        .map(|e| convert_event(e, calendar_id))
        .collect()
}

fn convert_event(event: Event, calendar_id: &str) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();

    let started_at = event
        .start
        .as_ref()
        .map(|start| start.date_time.clone())
        .unwrap_or_default();
    let ended_at = event
        .end
        .as_ref()
        .map(|end| end.date_time.clone())
        .unwrap_or_default();
    let timezone = event
        .start
        .as_ref()
        .and_then(|start| start.time_zone.clone());

    let organizer = event.organizer.as_ref().map(|organizer| EventPerson {
        name: organizer
            .email_address
            .as_ref()
            .and_then(|email| email.name.clone()),
        email: organizer
            .email_address
            .as_ref()
            .and_then(|email| email.address.clone()),
        is_current_user: event.is_organizer.unwrap_or(false),
    });

    let attendees = event
        .attendees
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(convert_attendee)
        .collect();

    let meeting_link = event.online_meeting_url.clone().or_else(|| {
        event
            .online_meeting
            .as_ref()
            .and_then(|meeting| meeting.join_url.clone())
    });

    CalendarEvent {
        id: event.id,
        calendar_id: calendar_id.to_string(),
        provider: CalendarProviderType::Outlook,
        external_id: event.ical_uid.unwrap_or_default(),
        title: event.subject.unwrap_or_default(),
        description: event.body.and_then(|body| body.content),
        location: event.location.and_then(|location| location.display_name),
        url: event.web_link,
        meeting_link,
        started_at,
        ended_at,
        timezone,
        is_all_day: event.is_all_day.unwrap_or(false),
        status: convert_status(event.is_cancelled, event.show_as),
        organizer,
        attendees,
        has_recurrence_rules: event.recurrence.is_some() || event.series_master_id.is_some(),
        recurring_event_id: event.series_master_id,
        raw,
    }
}

fn convert_status(is_cancelled: Option<bool>, show_as: Option<EventShowAs>) -> EventStatus {
    if is_cancelled.unwrap_or(false) {
        EventStatus::Cancelled
    } else if matches!(show_as, Some(EventShowAs::Tentative)) {
        EventStatus::Tentative
    } else {
        EventStatus::Confirmed
    }
}

fn convert_attendee(attendee: &Attendee) -> EventAttendee {
    EventAttendee {
        name: attendee
            .email_address
            .as_ref()
            .and_then(|email| email.name.clone()),
        email: attendee
            .email_address
            .as_ref()
            .and_then(|email| email.address.clone()),
        is_current_user: false,
        status: convert_attendee_status(attendee),
        role: convert_attendee_role(attendee.type_.as_ref()),
    }
}

fn convert_attendee_status(attendee: &Attendee) -> AttendeeStatus {
    match attendee
        .status
        .as_ref()
        .and_then(|status| status.response.as_ref())
    {
        Some(OutlookResponseType::Accepted) | Some(OutlookResponseType::Organizer) => {
            AttendeeStatus::Accepted
        }
        Some(OutlookResponseType::TentativelyAccepted) => AttendeeStatus::Tentative,
        Some(OutlookResponseType::Declined) => AttendeeStatus::Declined,
        _ => AttendeeStatus::Pending,
    }
}

fn convert_attendee_role(role: Option<&AttendeeType>) -> AttendeeRole {
    match role {
        Some(AttendeeType::Optional) => AttendeeRole::Optional,
        Some(AttendeeType::Resource) => AttendeeRole::NonParticipant,
        _ => AttendeeRole::Required,
    }
}
