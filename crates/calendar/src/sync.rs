use std::collections::{HashMap, HashSet};

use hypr_calendar_interface::{AttendeeStatus, CalendarEvent};

#[derive(Debug, Clone)]
pub struct IncomingEvent {
    pub tracking_id_event: String,
    pub tracking_id_calendar: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: String,
    pub meeting_link: String,
    pub description: String,
    pub recurrence_series_id: String,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
}

#[derive(Debug, Clone)]
pub struct EventParticipant {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_organizer: bool,
    pub is_current_user: bool,
}

#[derive(Debug, Clone)]
pub struct ExistingEvent {
    pub id: String,
    pub user_id: String,
    pub calendar_id: String,
    pub tracking_id_event: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: String,
    pub meeting_link: String,
    pub description: String,
    pub note: String,
    pub recurrence_series_id: String,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub participants_json: String,
    pub raw_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EventToAdd {
    pub incoming: IncomingEvent,
    pub participants: Vec<EventParticipant>,
}

#[derive(Debug, Clone)]
pub struct EventToUpdate {
    pub id: String,
    pub user_id: String,
    pub calendar_id: String,
    pub created_at: String,
    pub incoming: IncomingEvent,
    pub participants: Vec<EventParticipant>,
}

#[derive(Debug, Default)]
pub struct SyncPlan {
    pub to_add: Vec<EventToAdd>,
    pub to_update: Vec<EventToUpdate>,
    pub to_delete: Vec<String>,
}

pub type IncomingParticipants = HashMap<String, Vec<EventParticipant>>;

pub fn compute_sync_plan(
    incoming: &[IncomingEvent],
    existing: &[ExistingEvent],
    incoming_participants: &IncomingParticipants,
) -> SyncPlan {
    let mut plan = SyncPlan::default();

    let incoming_by_tracking_id: HashMap<&str, &IncomingEvent> = incoming
        .iter()
        .map(|e| (e.tracking_id_event.as_str(), e))
        .collect();
    let mut handled_tracking_ids = HashSet::new();

    for store_event in existing {
        let tracking_id = &store_event.tracking_id_event;
        let matching = if !tracking_id.is_empty() {
            incoming_by_tracking_id.get(tracking_id.as_str()).copied()
        } else {
            None
        };

        if let Some(matched) = matching {
            let participants = incoming_participants
                .get(tracking_id)
                .cloned()
                .unwrap_or_default();
            plan.to_update.push(EventToUpdate {
                id: store_event.id.clone(),
                user_id: store_event.user_id.clone(),
                calendar_id: store_event.calendar_id.clone(),
                created_at: store_event.created_at.clone(),
                incoming: matched.clone(),
                participants,
            });
            handled_tracking_ids.insert(tracking_id.as_str());
        } else {
            plan.to_delete.push(store_event.id.clone());
        }
    }

    for incoming_event in incoming {
        if !handled_tracking_ids.contains(incoming_event.tracking_id_event.as_str()) {
            let participants = incoming_participants
                .get(&incoming_event.tracking_id_event)
                .cloned()
                .unwrap_or_default();
            plan.to_add.push(EventToAdd {
                incoming: incoming_event.clone(),
                participants,
            });
        }
    }

    plan
}

pub fn normalize_calendar_event(
    event: &CalendarEvent,
) -> Option<(IncomingEvent, Vec<EventParticipant>)> {
    let declined = event
        .attendees
        .iter()
        .any(|a| a.is_current_user && a.status == AttendeeStatus::Declined);
    if declined {
        return None;
    }

    let mut participants = Vec::new();

    if let Some(ref organizer) = event.organizer {
        participants.push(EventParticipant {
            name: organizer.name.clone(),
            email: organizer.email.clone(),
            is_organizer: true,
            is_current_user: organizer.is_current_user,
        });
    }

    let organizer_email = event
        .organizer
        .as_ref()
        .and_then(|o| o.email.as_deref())
        .map(|e| e.to_lowercase());

    for attendee in &event.attendees {
        if attendee.role == hypr_calendar_interface::AttendeeRole::NonParticipant {
            continue;
        }
        if let Some(ref org_email) = organizer_email {
            if attendee
                .email
                .as_deref()
                .map(|e| e.to_lowercase())
                .as_deref()
                == Some(org_email)
            {
                continue;
            }
        }
        participants.push(EventParticipant {
            name: attendee.name.clone(),
            email: attendee.email.clone(),
            is_organizer: false,
            is_current_user: attendee.is_current_user,
        });
    }

    let incoming = IncomingEvent {
        tracking_id_event: event.id.clone(),
        tracking_id_calendar: event.calendar_id.clone(),
        title: event.title.clone(),
        started_at: event.started_at.clone(),
        ended_at: event.ended_at.clone(),
        location: event.location.clone().unwrap_or_default(),
        meeting_link: event.meeting_link.clone().unwrap_or_default(),
        description: event.description.clone().unwrap_or_default(),
        recurrence_series_id: event.recurring_event_id.clone().unwrap_or_default(),
        has_recurrence_rules: event.has_recurrence_rules,
        is_all_day: event.is_all_day,
    };

    Some((incoming, participants))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_incoming(tracking_id: &str, title: &str) -> IncomingEvent {
        IncomingEvent {
            tracking_id_event: tracking_id.to_string(),
            tracking_id_calendar: "cal-1".to_string(),
            title: title.to_string(),
            started_at: "2026-03-19T09:00:00Z".to_string(),
            ended_at: "2026-03-19T10:00:00Z".to_string(),
            location: String::new(),
            meeting_link: String::new(),
            description: String::new(),
            recurrence_series_id: String::new(),
            has_recurrence_rules: false,
            is_all_day: false,
        }
    }

    fn make_existing(id: &str, tracking_id: &str) -> ExistingEvent {
        ExistingEvent {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            calendar_id: "cal-local-1".to_string(),
            tracking_id_event: tracking_id.to_string(),
            title: "Old Title".to_string(),
            started_at: "2026-03-19T09:00:00Z".to_string(),
            ended_at: "2026-03-19T10:00:00Z".to_string(),
            location: String::new(),
            meeting_link: String::new(),
            description: String::new(),
            note: String::new(),
            recurrence_series_id: String::new(),
            has_recurrence_rules: false,
            is_all_day: false,
            participants_json: "[]".to_string(),
            raw_json: "{}".to_string(),
            created_at: "2026-03-18T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn new_events_get_added() {
        let incoming = vec![make_incoming("t1", "Meeting A")];
        let existing = vec![];
        let participants = HashMap::new();

        let plan = compute_sync_plan(&incoming, &existing, &participants);

        assert_eq!(plan.to_add.len(), 1);
        assert_eq!(plan.to_add[0].incoming.title, "Meeting A");
        assert!(plan.to_update.is_empty());
        assert!(plan.to_delete.is_empty());
    }

    #[test]
    fn matching_events_get_updated() {
        let incoming = vec![make_incoming("t1", "Updated Meeting")];
        let existing = vec![make_existing("local-1", "t1")];
        let participants = HashMap::new();

        let plan = compute_sync_plan(&incoming, &existing, &participants);

        assert!(plan.to_add.is_empty());
        assert_eq!(plan.to_update.len(), 1);
        assert_eq!(plan.to_update[0].id, "local-1");
        assert_eq!(plan.to_update[0].incoming.title, "Updated Meeting");
        assert!(plan.to_delete.is_empty());
    }

    #[test]
    fn orphaned_events_get_deleted() {
        let incoming = vec![];
        let existing = vec![make_existing("local-1", "t1")];
        let participants = HashMap::new();

        let plan = compute_sync_plan(&incoming, &existing, &participants);

        assert!(plan.to_add.is_empty());
        assert!(plan.to_update.is_empty());
        assert_eq!(plan.to_delete.len(), 1);
        assert_eq!(plan.to_delete[0], "local-1");
    }

    #[test]
    fn mixed_add_update_delete() {
        let incoming = vec![
            make_incoming("t1", "Updated"),
            make_incoming("t3", "New Event"),
        ];
        let existing = vec![
            make_existing("local-1", "t1"),
            make_existing("local-2", "t2"),
        ];
        let participants = HashMap::new();

        let plan = compute_sync_plan(&incoming, &existing, &participants);

        assert_eq!(plan.to_add.len(), 1);
        assert_eq!(plan.to_add[0].incoming.tracking_id_event, "t3");
        assert_eq!(plan.to_update.len(), 1);
        assert_eq!(plan.to_update[0].id, "local-1");
        assert_eq!(plan.to_delete.len(), 1);
        assert_eq!(plan.to_delete[0], "local-2");
    }

    #[test]
    fn declined_events_skipped_in_normalize() {
        use hypr_calendar_interface::*;

        let event = CalendarEvent {
            provider: CalendarProviderType::Google,
            id: "e1".to_string(),
            calendar_id: "cal1".to_string(),
            external_id: "ext1".to_string(),
            title: "Declined Meeting".to_string(),
            description: None,
            location: None,
            url: None,
            meeting_link: None,
            started_at: "2026-03-19T09:00:00Z".to_string(),
            ended_at: "2026-03-19T10:00:00Z".to_string(),
            timezone: None,
            is_all_day: false,
            status: EventStatus::Confirmed,
            organizer: None,
            attendees: vec![EventAttendee {
                name: Some("Me".to_string()),
                email: Some("me@example.com".to_string()),
                is_current_user: true,
                status: AttendeeStatus::Declined,
                role: AttendeeRole::Required,
            }],
            has_recurrence_rules: false,
            recurring_event_id: None,
            raw: "{}".to_string(),
        };

        assert!(normalize_calendar_event(&event).is_none());
    }

    #[test]
    fn normalize_extracts_participants() {
        use hypr_calendar_interface::*;

        let event = CalendarEvent {
            provider: CalendarProviderType::Google,
            id: "e1".to_string(),
            calendar_id: "cal1".to_string(),
            external_id: "ext1".to_string(),
            title: "Team Meeting".to_string(),
            description: None,
            location: Some("Room A".to_string()),
            url: None,
            meeting_link: Some("https://meet.example.com".to_string()),
            started_at: "2026-03-19T09:00:00Z".to_string(),
            ended_at: "2026-03-19T10:00:00Z".to_string(),
            timezone: None,
            is_all_day: false,
            status: EventStatus::Confirmed,
            organizer: Some(EventPerson {
                name: Some("Boss".to_string()),
                email: Some("boss@example.com".to_string()),
                is_current_user: false,
            }),
            attendees: vec![
                EventAttendee {
                    name: Some("Boss".to_string()),
                    email: Some("boss@example.com".to_string()),
                    is_current_user: false,
                    status: AttendeeStatus::Accepted,
                    role: AttendeeRole::Chair,
                },
                EventAttendee {
                    name: Some("Me".to_string()),
                    email: Some("me@example.com".to_string()),
                    is_current_user: true,
                    status: AttendeeStatus::Accepted,
                    role: AttendeeRole::Required,
                },
                EventAttendee {
                    name: Some("Observer".to_string()),
                    email: Some("observer@example.com".to_string()),
                    is_current_user: false,
                    status: AttendeeStatus::Accepted,
                    role: AttendeeRole::NonParticipant,
                },
            ],
            has_recurrence_rules: false,
            recurring_event_id: None,
            raw: "{}".to_string(),
        };

        let (incoming, participants) = normalize_calendar_event(&event).unwrap();
        assert_eq!(incoming.title, "Team Meeting");
        assert_eq!(incoming.location, "Room A");
        // organizer + me (boss duplicate skipped, observer nonparticipant skipped)
        assert_eq!(participants.len(), 2);
        assert!(participants[0].is_organizer);
        assert_eq!(participants[0].email.as_deref(), Some("boss@example.com"));
        assert!(participants[1].is_current_user);
    }

    #[test]
    fn participants_attached_to_sync_plan() {
        let incoming = vec![make_incoming("t1", "Meeting")];
        let existing = vec![];
        let mut participants = HashMap::new();
        participants.insert(
            "t1".to_string(),
            vec![EventParticipant {
                name: Some("Alice".to_string()),
                email: Some("alice@example.com".to_string()),
                is_organizer: false,
                is_current_user: false,
            }],
        );

        let plan = compute_sync_plan(&incoming, &existing, &participants);

        assert_eq!(plan.to_add.len(), 1);
        assert_eq!(plan.to_add[0].participants.len(), 1);
        assert_eq!(
            plan.to_add[0].participants[0].email.as_deref(),
            Some("alice@example.com")
        );
    }
}
