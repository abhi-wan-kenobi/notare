//! VCALENDAR/VEVENT parsing on top of the `ical` crate.
//!
//! The `ical` crate handles the syntax layer (line unfolding, properties,
//! params); this module resolves the semantics we need: datetimes (UTC "Z",
//! TZID-qualified, floating, and all-day DATE values), text unescaping, and
//! the properties Notare surfaces (SUMMARY, DESCRIPTION, LOCATION, URL,
//! ORGANIZER, ATTENDEE, STATUS, UID, RRULE/EXDATE/RDATE, RECURRENCE-ID).

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::parser::ical::component::IcalEvent;
use ical::property::Property;

use crate::Error;

#[derive(Debug, Clone)]
pub struct IcsCalendar {
    /// `X-WR-CALNAME`, when present.
    pub name: Option<String>,
    pub events: Vec<IcsEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcsEventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IcsPerson {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IcsAttendeeStatus {
    Pending,
    Accepted,
    Tentative,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IcsAttendeeRole {
    Chair,
    Required,
    Optional,
    NonParticipant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IcsAttendee {
    pub name: Option<String>,
    pub email: Option<String>,
    pub status: IcsAttendeeStatus,
    pub role: IcsAttendeeRole,
}

#[derive(Debug, Clone)]
pub struct IcsEvent {
    pub uid: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,

    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub is_all_day: bool,
    /// `TZID` of DTSTART, when present.
    pub timezone: Option<String>,

    pub status: IcsEventStatus,
    pub organizer: Option<IcsPerson>,
    pub attendees: Vec<IcsAttendee>,

    /// Raw `RRULE` value (e.g. `FREQ=WEEKLY;BYDAY=MO`).
    pub rrule: Option<String>,
    /// Raw content lines (`NAME;PARAMS:VALUE`) for recurrence reconstruction.
    pub dtstart_line: String,
    pub exdate_lines: Vec<String>,
    pub rdate_lines: Vec<String>,
    /// Set when this VEVENT overrides one instance of a recurring series.
    pub recurrence_id: Option<DateTime<Utc>>,

    /// JSON dump of all raw properties (for the `raw` column downstream).
    pub raw: String,
}

/// Parse a full `.ics` document. Fails cleanly on syntactically broken input
/// and on files without a single VEVENT.
pub fn parse_ics(text: &str) -> Result<IcsCalendar, Error> {
    let reader = std::io::BufReader::new(text.as_bytes());
    let mut parser = ical::IcalParser::new(reader);

    let calendar = match parser.next() {
        Some(Ok(calendar)) => calendar,
        Some(Err(e)) => return Err(Error::Parse(e.to_string())),
        None => return Err(Error::Parse("no VCALENDAR component found".into())),
    };

    let name = find_value(&calendar.properties, "X-WR-CALNAME").map(unescape_text);

    let mut events = Vec::new();
    for event in &calendar.events {
        match parse_event(event) {
            Ok(parsed) => events.push(parsed),
            Err(e) => {
                // One bad VEVENT should not take the whole calendar down.
                tracing::warn!("skipping unparsable VEVENT: {e}");
            }
        }
    }

    // Additional VCALENDARs in the same file (rare but legal) are merged in.
    for extra in parser {
        let extra = match extra {
            Ok(extra) => extra,
            Err(e) => {
                tracing::warn!("skipping trailing VCALENDAR: {e}");
                continue;
            }
        };
        for event in &extra.events {
            match parse_event(event) {
                Ok(parsed) => events.push(parsed),
                Err(e) => tracing::warn!("skipping unparsable VEVENT: {e}"),
            }
        }
    }

    if events.is_empty() {
        return Err(Error::NoEvents);
    }

    Ok(IcsCalendar { name, events })
}

fn parse_event(event: &IcalEvent) -> Result<IcsEvent, Error> {
    let uid = find_value(&event.properties, "UID")
        .map(unescape_text)
        .ok_or_else(|| Error::Parse("VEVENT is missing UID".into()))?;

    let dtstart = find_property(&event.properties, "DTSTART")
        .ok_or_else(|| Error::Parse(format!("VEVENT {uid} is missing DTSTART")))?;
    let (start, is_all_day, timezone) = parse_datetime_property(dtstart)?;

    let end = match find_property(&event.properties, "DTEND") {
        Some(dtend) => parse_datetime_property(dtend)?.0,
        None => match find_value(&event.properties, "DURATION") {
            Some(duration) => start + parse_duration(&duration)?,
            // RFC 5545 3.6.1: no DTEND/DURATION -> all-day events span one
            // day, timed events have zero duration.
            None if is_all_day => start + Duration::days(1),
            None => start,
        },
    };

    let status = match find_value(&event.properties, "STATUS").as_deref() {
        Some("TENTATIVE") => IcsEventStatus::Tentative,
        Some("CANCELLED") => IcsEventStatus::Cancelled,
        _ => IcsEventStatus::Confirmed,
    };

    let organizer = find_property(&event.properties, "ORGANIZER").map(parse_person);
    let attendees = event
        .properties
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case("ATTENDEE"))
        .map(parse_attendee)
        .collect();

    let recurrence_id = match find_property(&event.properties, "RECURRENCE-ID") {
        Some(prop) => Some(parse_datetime_property(prop)?.0),
        None => None,
    };

    let raw = serde_json::to_string(&properties_to_json(&event.properties)).unwrap_or_default();

    Ok(IcsEvent {
        summary: find_value(&event.properties, "SUMMARY").map(unescape_text),
        description: find_value(&event.properties, "DESCRIPTION").map(unescape_text),
        location: find_value(&event.properties, "LOCATION").map(unescape_text),
        url: find_value(&event.properties, "URL"),
        start,
        end: end.max(start),
        is_all_day,
        timezone,
        status,
        organizer,
        attendees,
        rrule: find_value(&event.properties, "RRULE"),
        dtstart_line: property_to_line(dtstart),
        exdate_lines: properties_to_lines(&event.properties, "EXDATE"),
        rdate_lines: properties_to_lines(&event.properties, "RDATE"),
        recurrence_id,
        raw,
        uid,
    })
}

// --- property helpers ---

fn find_property<'a>(properties: &'a [Property], name: &str) -> Option<&'a Property> {
    properties
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

fn find_value(properties: &[Property], name: &str) -> Option<String> {
    find_property(properties, name).and_then(|p| p.value.clone())
}

fn param_value<'a>(property: &'a Property, name: &str) -> Option<&'a str> {
    property.params.as_ref()?.iter().find_map(|(key, values)| {
        if key.eq_ignore_ascii_case(name) {
            values.first().map(String::as_str)
        } else {
            None
        }
    })
}

/// Re-serialize a parsed property as a content line (`NAME;P=V:value`) —
/// used to hand recurrence data to the `rrule` crate verbatim.
fn property_to_line(property: &Property) -> String {
    let mut line = property.name.clone();
    if let Some(params) = &property.params {
        for (key, values) in params {
            line.push(';');
            line.push_str(key);
            line.push('=');
            line.push_str(&values.join(","));
        }
    }
    line.push(':');
    if let Some(value) = &property.value {
        line.push_str(value);
    }
    line
}

fn properties_to_lines(properties: &[Property], name: &str) -> Vec<String> {
    properties
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(name))
        .map(property_to_line)
        .collect()
}

fn properties_to_json(properties: &[Property]) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = properties
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                serde_json::Value::String(p.value.clone().unwrap_or_default()),
            )
        })
        .collect();
    serde_json::Value::Object(map)
}

// --- datetime handling ---

/// Resolve a DTSTART/DTEND/RECURRENCE-ID property to UTC.
///
/// Handles, in order: `VALUE=DATE` / bare `YYYYMMDD` (all-day, anchored at
/// UTC midnight — same convention the Google conversion uses), `...Z` (UTC),
/// `TZID=...` (IANA zone via chrono-tz), and floating times (interpreted in
/// the machine's local timezone, per RFC 5545's "local time" reading).
fn parse_datetime_property(
    property: &Property,
) -> Result<(DateTime<Utc>, bool, Option<String>), Error> {
    let value = property
        .value
        .as_deref()
        .ok_or_else(|| Error::Parse(format!("{} has no value", property.name)))?
        .trim();
    let tzid = param_value(property, "TZID").map(str::to_string);
    let is_date = param_value(property, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE"))
        || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()));

    if is_date {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| Error::Parse(format!("invalid DATE value {value}: {e}")))?;
        let midnight = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| Error::Parse(format!("invalid DATE value {value}")))?;
        return Ok((Utc.from_utc_datetime(&midnight), true, tzid));
    }

    if let Some(stripped) = value.strip_suffix('Z') {
        let naive = parse_naive_datetime(stripped)?;
        return Ok((Utc.from_utc_datetime(&naive), false, tzid));
    }

    let naive = parse_naive_datetime(value)?;

    if let Some(tz_name) = &tzid {
        let tz: chrono_tz::Tz = tz_name
            .parse()
            .map_err(|_| Error::Parse(format!("unknown TZID {tz_name}")))?;
        let resolved = tz
            .from_local_datetime(&naive)
            .earliest()
            .ok_or_else(|| Error::Parse(format!("nonexistent local time {value} in {tz_name}")))?;
        return Ok((resolved.with_timezone(&Utc), false, tzid));
    }

    // Floating time: interpret in the machine's local timezone.
    let resolved = chrono::Local
        .from_local_datetime(&naive)
        .earliest()
        .ok_or_else(|| Error::Parse(format!("nonexistent local time {value}")))?;
    Ok((resolved.with_timezone(&Utc), false, None))
}

fn parse_naive_datetime(value: &str) -> Result<NaiveDateTime, Error> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|e| Error::Parse(format!("invalid DATE-TIME value {value}: {e}")))
}

/// Minimal RFC 5545 DURATION parser (`P1D`, `PT1H30M`, `-PT15M`, `P1W`).
fn parse_duration(value: &str) -> Result<Duration, Error> {
    let err = || Error::Parse(format!("invalid DURATION value {value}"));
    let mut rest = value.trim();
    let negative = if let Some(stripped) = rest.strip_prefix('-') {
        rest = stripped;
        true
    } else {
        rest = rest.strip_prefix('+').unwrap_or(rest);
        false
    };
    rest = rest.strip_prefix('P').ok_or_else(err)?;

    let mut total = Duration::zero();
    let mut in_time = false;
    let mut digits = String::new();
    for c in rest.chars() {
        match c {
            'T' | 't' => in_time = true,
            '0'..='9' => digits.push(c),
            unit => {
                let n: i64 = digits.parse().map_err(|_| err())?;
                digits.clear();
                total += match (unit.to_ascii_uppercase(), in_time) {
                    ('W', false) => Duration::weeks(n),
                    ('D', false) => Duration::days(n),
                    ('H', true) => Duration::hours(n),
                    ('M', true) => Duration::minutes(n),
                    ('S', true) => Duration::seconds(n),
                    _ => return Err(err()),
                };
            }
        }
    }
    if !digits.is_empty() {
        return Err(err());
    }
    Ok(if negative { -total } else { total })
}

// --- people ---

fn parse_person(property: &Property) -> IcsPerson {
    IcsPerson {
        name: param_value(property, "CN").map(str::to_string),
        email: mailto_email(property.value.as_deref()),
    }
}

fn parse_attendee(property: &Property) -> IcsAttendee {
    let status = match param_value(property, "PARTSTAT")
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("ACCEPTED") | Some("DELEGATED") | Some("COMPLETED") | Some("IN-PROCESS") => {
            IcsAttendeeStatus::Accepted
        }
        Some("TENTATIVE") => IcsAttendeeStatus::Tentative,
        Some("DECLINED") => IcsAttendeeStatus::Declined,
        _ => IcsAttendeeStatus::Pending,
    };
    let role = match param_value(property, "ROLE")
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("CHAIR") => IcsAttendeeRole::Chair,
        Some("OPT-PARTICIPANT") => IcsAttendeeRole::Optional,
        Some("NON-PARTICIPANT") => IcsAttendeeRole::NonParticipant,
        _ => IcsAttendeeRole::Required,
    };

    IcsAttendee {
        name: param_value(property, "CN").map(str::to_string),
        email: mailto_email(property.value.as_deref()),
        status,
        role,
    }
}

fn mailto_email(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let email = value
        .strip_prefix("mailto:")
        .or_else(|| value.strip_prefix("MAILTO:"))
        .unwrap_or(value);
    if email.contains('@') {
        Some(email.to_string())
    } else {
        None
    }
}

/// RFC 5545 3.3.11 TEXT unescaping.
fn unescape_text(value: String) -> String {
    if !value.contains('\\') {
        return value;
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(escaped) => out.push(escaped),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n{body}END:VCALENDAR\r\n")
    }

    fn single_event(body: &str) -> IcsEvent {
        let ics = wrap(&format!("BEGIN:VEVENT\r\n{body}END:VEVENT\r\n"));
        let calendar = parse_ics(&ics).expect("parse");
        assert_eq!(calendar.events.len(), 1);
        calendar.events.into_iter().next().unwrap()
    }

    #[test]
    fn parses_utc_z_suffix_event() {
        let event = single_event(
            "UID:evt-1\r\nDTSTART:20260117T090000Z\r\nDTEND:20260117T100000Z\r\nSUMMARY:Standup\r\n",
        );
        assert_eq!(event.start.to_rfc3339(), "2026-01-17T09:00:00+00:00");
        assert_eq!(event.end.to_rfc3339(), "2026-01-17T10:00:00+00:00");
        assert!(!event.is_all_day);
        assert_eq!(event.summary.as_deref(), Some("Standup"));
    }

    #[test]
    fn parses_tzid_event() {
        let event = single_event(
            "UID:evt-2\r\nDTSTART;TZID=Asia/Kolkata:20260117T093000\r\nDTEND;TZID=Asia/Kolkata:20260117T103000\r\nSUMMARY:IST meeting\r\n",
        );
        // 09:30 IST == 04:00 UTC
        assert_eq!(event.start.to_rfc3339(), "2026-01-17T04:00:00+00:00");
        assert_eq!(event.timezone.as_deref(), Some("Asia/Kolkata"));
    }

    #[test]
    fn parses_all_day_event() {
        let event = single_event(
            "UID:evt-3\r\nDTSTART;VALUE=DATE:20260201\r\nDTEND;VALUE=DATE:20260203\r\nSUMMARY:Offsite\r\n",
        );
        assert!(event.is_all_day);
        assert_eq!(event.start.to_rfc3339(), "2026-02-01T00:00:00+00:00");
        assert_eq!(event.end.to_rfc3339(), "2026-02-03T00:00:00+00:00");
    }

    #[test]
    fn all_day_without_dtend_spans_one_day() {
        let event = single_event("UID:evt-4\r\nDTSTART;VALUE=DATE:20260201\r\nSUMMARY:Holiday\r\n");
        assert!(event.is_all_day);
        assert_eq!(event.end - event.start, Duration::days(1));
    }

    #[test]
    fn timed_without_dtend_has_zero_duration() {
        let event = single_event("UID:evt-5\r\nDTSTART:20260117T090000Z\r\nSUMMARY:Ping\r\n");
        assert_eq!(event.start, event.end);
    }

    #[test]
    fn duration_is_used_when_dtend_missing() {
        let event = single_event(
            "UID:evt-6\r\nDTSTART:20260117T090000Z\r\nDURATION:PT1H30M\r\nSUMMARY:Review\r\n",
        );
        assert_eq!(event.end - event.start, Duration::minutes(90));
    }

    #[test]
    fn parses_people_status_and_text_escapes() {
        let event = single_event(
            "UID:evt-7\r\nDTSTART:20260117T090000Z\r\nDTEND:20260117T100000Z\r\nSUMMARY:A\\, B\\nC\r\nDESCRIPTION:line1\\nline2\r\nLOCATION:Room 4\r\nSTATUS:TENTATIVE\r\nORGANIZER;CN=Alice:mailto:alice@example.com\r\nATTENDEE;CN=Bob;PARTSTAT=ACCEPTED;ROLE=OPT-PARTICIPANT:mailto:bob@example.com\r\nATTENDEE;PARTSTAT=DECLINED:mailto:carol@example.com\r\n",
        );
        assert_eq!(event.summary.as_deref(), Some("A, B\nC"));
        assert_eq!(event.description.as_deref(), Some("line1\nline2"));
        assert_eq!(event.status, IcsEventStatus::Tentative);
        let organizer = event.organizer.expect("organizer");
        assert_eq!(organizer.name.as_deref(), Some("Alice"));
        assert_eq!(organizer.email.as_deref(), Some("alice@example.com"));
        assert_eq!(event.attendees.len(), 2);
        assert_eq!(event.attendees[0].status, IcsAttendeeStatus::Accepted);
        assert_eq!(event.attendees[0].role, IcsAttendeeRole::Optional);
        assert_eq!(event.attendees[1].status, IcsAttendeeStatus::Declined);
        assert_eq!(event.attendees[1].role, IcsAttendeeRole::Required);
    }

    #[test]
    fn reads_calendar_name_and_rrule() {
        let ics = wrap(
            "X-WR-CALNAME:Team Calendar\r\nBEGIN:VEVENT\r\nUID:evt-8\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T093000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Weekly\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).expect("parse");
        assert_eq!(calendar.name.as_deref(), Some("Team Calendar"));
        assert_eq!(
            calendar.events[0].rrule.as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO")
        );
        assert_eq!(calendar.events[0].dtstart_line, "DTSTART:20260105T090000Z");
    }

    #[test]
    fn malformed_file_is_a_clean_error() {
        assert!(matches!(
            parse_ics("this is not an ics file"),
            Err(Error::Parse(_)) | Err(Error::NoEvents)
        ));
        // Structurally valid but empty calendar -> NoEvents.
        assert!(matches!(
            parse_ics("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n"),
            Err(Error::NoEvents)
        ));
    }

    #[test]
    fn event_missing_dtstart_is_skipped_not_fatal() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:bad\r\nSUMMARY:No start\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:good\r\nDTSTART:20260117T090000Z\r\nSUMMARY:Fine\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).expect("parse");
        assert_eq!(calendar.events.len(), 1);
        assert_eq!(calendar.events[0].uid, "good");
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("P1D").unwrap(), Duration::days(1));
        assert_eq!(parse_duration("P1W").unwrap(), Duration::weeks(1));
        assert_eq!(parse_duration("PT1H30M").unwrap(), Duration::minutes(90));
        assert_eq!(parse_duration("-PT15M").unwrap(), Duration::minutes(-15));
        assert!(parse_duration("1H").is_err());
    }
}
