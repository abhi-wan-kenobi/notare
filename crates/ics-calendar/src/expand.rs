//! Expansion of parsed VEVENTs into concrete occurrences inside a window.
//!
//! Recurring events (RRULE, plus EXDATE/RDATE) are expanded with the `rrule`
//! crate by re-assembling the original content lines. When an RRULE cannot be
//! parsed/expanded (exotic rules, non-IANA TZIDs), we degrade to the single
//! DTSTART instance and log a warning — that limitation is documented in
//! `docs/ICS-IMPORT.md`.
//!
//! `RECURRENCE-ID` override VEVENTs are honored: the overridden occurrence of
//! the master is suppressed and the override is emitted as its own event
//! (unless the override is CANCELLED).

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use crate::parse::{IcsCalendar, IcsEvent};

/// Hard cap on occurrences expanded per event per query (windows are ~days,
/// so this is only a runaway guard).
const MAX_OCCURRENCES: u16 = 1000;

#[derive(Debug, Clone, Copy)]
pub struct ExpandOptions {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// One concrete event instance inside the query window.
#[derive(Debug, Clone)]
pub struct IcsOccurrence {
    /// Unique between occurrences: the UID for single events,
    /// `UID:<rfc3339 start>` for expanded/overridden recurring instances.
    pub id: String,
    pub event: IcsEvent,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// True when this instance belongs to a recurring series.
    pub has_recurrence_rules: bool,
    /// The series UID for recurring instances.
    pub series_id: Option<String>,
}

/// Expand every event of `calendar` into occurrences overlapping the window.
pub fn expand_events(calendar: &IcsCalendar, options: ExpandOptions) -> Vec<IcsOccurrence> {
    let mut occurrences = Vec::new();

    // Starts of instances that are overridden by RECURRENCE-ID VEVENTs,
    // keyed per series UID.
    let mut overridden: HashSet<(String, DateTime<Utc>)> = HashSet::new();
    for event in &calendar.events {
        if let Some(recurrence_id) = event.recurrence_id {
            overridden.insert((event.uid.clone(), recurrence_id));
        }
    }

    for event in &calendar.events {
        if let Some(recurrence_id) = event.recurrence_id {
            // Override instance: emit directly (skip cancelled ones).
            if event.status == crate::parse::IcsEventStatus::Cancelled {
                continue;
            }
            if overlaps(event.start, event.end, options) {
                occurrences.push(IcsOccurrence {
                    id: format!("{}:{}", event.uid, event.start.to_rfc3339()),
                    event: event.clone(),
                    start: event.start,
                    end: event.end,
                    has_recurrence_rules: true,
                    series_id: Some(event.uid.clone()),
                });
            }
            let _ = recurrence_id;
            continue;
        }

        let is_recurring =
            event.rrule.is_some() || !event.rdate_lines.is_empty();

        if !is_recurring {
            if overlaps(event.start, event.end, options) {
                occurrences.push(IcsOccurrence {
                    id: event.uid.clone(),
                    event: event.clone(),
                    start: event.start,
                    end: event.end,
                    has_recurrence_rules: false,
                    series_id: None,
                });
            }
            continue;
        }

        let duration = event.end - event.start;
        let starts = match expand_rrule(event, options, duration) {
            Ok(starts) => starts,
            Err(e) => {
                tracing::warn!(
                    uid = %event.uid,
                    "could not expand recurrence ({e}); falling back to the first instance only"
                );
                vec![event.start]
            }
        };

        for start in starts {
            if overridden.contains(&(event.uid.clone(), start)) {
                continue;
            }
            let end = start + duration;
            if !overlaps(start, end, options) {
                continue;
            }
            occurrences.push(IcsOccurrence {
                id: format!("{}:{}", event.uid, start.to_rfc3339()),
                event: event.clone(),
                start,
                end,
                has_recurrence_rules: true,
                series_id: Some(event.uid.clone()),
            });
        }
    }

    occurrences.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    occurrences
}

fn overlaps(start: DateTime<Utc>, end: DateTime<Utc>, options: ExpandOptions) -> bool {
    start < options.to && end.max(start) > options.from
}

/// Expand RRULE/RDATE/EXDATE via the `rrule` crate by rebuilding the
/// original content lines. All-day DTSTARTs are normalized to a UTC
/// DATE-TIME first (the crate expects DATE-TIME values).
fn expand_rrule(
    event: &IcsEvent,
    options: ExpandOptions,
    duration: Duration,
) -> Result<Vec<DateTime<Utc>>, String> {
    let mut lines: Vec<String> = Vec::new();

    if event.is_all_day {
        lines.push(format!(
            "DTSTART:{}",
            event.start.format("%Y%m%dT%H%M%SZ")
        ));
    } else {
        lines.push(event.dtstart_line.clone());
    }
    if let Some(rrule) = &event.rrule {
        lines.push(format!("RRULE:{rrule}"));
    }
    if !event.is_all_day {
        // EXDATE/RDATE for all-day series are DATE values the crate rejects;
        // skipping them is part of the documented degradation.
        lines.extend(event.exdate_lines.iter().cloned());
        lines.extend(event.rdate_lines.iter().cloned());
    }

    let set: rrule::RRuleSet = lines
        .join("\n")
        .parse()
        .map_err(|e: rrule::RRuleError| e.to_string())?;

    // Pull the window slightly wider so instances that started before the
    // window but still overlap it (start + duration > from) are kept.
    let lookback = duration.max(Duration::zero());
    let after = (options.from - lookback).with_timezone(&rrule::Tz::UTC);
    let before = options.to.with_timezone(&rrule::Tz::UTC);

    let result = set.after(after).before(before).all(MAX_OCCURRENCES);
    if result.limited {
        tracing::warn!(uid = %event.uid, "recurrence expansion hit the {MAX_OCCURRENCES} cap");
    }

    Ok(result
        .dates
        .into_iter()
        .map(|dt| dt.with_timezone(&Utc))
        .collect())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::parse::parse_ics;

    fn wrap(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n{body}END:VCALENDAR\r\n")
    }

    fn window(from: &str, to: &str) -> ExpandOptions {
        ExpandOptions {
            from: from.parse().unwrap(),
            to: to.parse().unwrap(),
        }
    }

    #[test]
    fn expands_basic_weekly_rrule_within_window() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:weekly-1\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T093000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Weekly sync\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-10T00:00:00Z", "2026-01-27T00:00:00Z"),
        );

        let starts: Vec<String> = occurrences.iter().map(|o| o.start.to_rfc3339()).collect();
        assert_eq!(
            starts,
            vec![
                "2026-01-12T09:00:00+00:00",
                "2026-01-19T09:00:00+00:00",
                "2026-01-26T09:00:00+00:00",
            ]
        );
        assert!(occurrences.iter().all(|o| o.has_recurrence_rules));
        assert!(
            occurrences
                .iter()
                .all(|o| o.series_id.as_deref() == Some("weekly-1"))
        );
        // Ids are unique per instance.
        let ids: std::collections::HashSet<_> = occurrences.iter().map(|o| &o.id).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn respects_exdate() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:weekly-2\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T093000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nEXDATE:20260119T090000Z\r\nSUMMARY:Weekly sync\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-10T00:00:00Z", "2026-01-27T00:00:00Z"),
        );
        let starts: Vec<String> = occurrences.iter().map(|o| o.start.to_rfc3339()).collect();
        assert_eq!(
            starts,
            vec!["2026-01-12T09:00:00+00:00", "2026-01-26T09:00:00+00:00"]
        );
    }

    #[test]
    fn includes_occurrence_overlapping_window_start() {
        // 2-hour event starting just before the window still overlaps it.
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:daily-1\r\nDTSTART:20260101T230000Z\r\nDTEND:20260102T010000Z\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Late show\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-03T00:00:00Z", "2026-01-04T00:00:00Z"),
        );
        let starts: Vec<String> = occurrences.iter().map(|o| o.start.to_rfc3339()).collect();
        assert_eq!(
            starts,
            vec!["2026-01-02T23:00:00+00:00", "2026-01-03T23:00:00+00:00"]
        );
    }

    #[test]
    fn single_events_pass_through_with_uid_id() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:single-1\r\nDTSTART:20260117T090000Z\r\nDTEND:20260117T100000Z\r\nSUMMARY:One-off\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-16T00:00:00Z", "2026-01-18T00:00:00Z"),
        );
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].id, "single-1");
        assert!(!occurrences[0].has_recurrence_rules);
        assert!(occurrences[0].series_id.is_none());

        let outside = expand_events(
            &calendar,
            window("2026-02-01T00:00:00Z", "2026-02-02T00:00:00Z"),
        );
        assert!(outside.is_empty());
    }

    #[test]
    fn recurrence_id_override_replaces_master_instance() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:weekly-3\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T093000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Weekly sync\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:weekly-3\r\nRECURRENCE-ID:20260112T090000Z\r\nDTSTART:20260112T140000Z\r\nDTEND:20260112T143000Z\r\nSUMMARY:Weekly sync (moved)\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-10T00:00:00Z", "2026-01-20T00:00:00Z"),
        );

        let starts: Vec<String> = occurrences.iter().map(|o| o.start.to_rfc3339()).collect();
        // 01-12 09:00 master instance replaced by the 14:00 override; 01-19 kept.
        assert_eq!(
            starts,
            vec!["2026-01-12T14:00:00+00:00", "2026-01-19T09:00:00+00:00"]
        );
        assert_eq!(
            occurrences[0].event.summary.as_deref(),
            Some("Weekly sync (moved)")
        );
    }

    #[test]
    fn unsupported_rrule_falls_back_to_first_instance() {
        let ics = wrap(
            "BEGIN:VEVENT\r\nUID:weird-1\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T093000Z\r\nRRULE:FREQ=BOGUS\r\nSUMMARY:Strange\r\nEND:VEVENT\r\n",
        );
        let calendar = parse_ics(&ics).unwrap();
        let occurrences = expand_events(
            &calendar,
            window("2026-01-01T00:00:00Z", "2026-02-01T00:00:00Z"),
        );
        assert_eq!(occurrences.len(), 1);
        assert_eq!(
            occurrences[0].start,
            Utc.with_ymd_and_hms(2026, 1, 5, 9, 0, 0).unwrap()
        );
    }
}
