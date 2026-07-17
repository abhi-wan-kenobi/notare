# Importing calendar files (.ics)

Notare can show events from any calendar you can export as an iCalendar
(`.ics`) file — Google Calendar exports, Outlook exports, university
timetables, sports fixtures, airline itineraries, etc. Everything is local:
no account, no network.

## How it works

1. Open **Settings → Calendar** (or the calendar step of onboarding) and
   expand **Calendar files (.ics)**.
2. Click **Import calendar file (.ics)…** and pick one or more files.
3. Each file becomes its own calendar. Toggle it on to sync its events into
   Notare like any other provider.

The picked file is **copied** into Notare's data directory
(`<app-data>/calendars/ics/`), so the original download can be moved or
deleted — Notare keeps serving events from its stored copy.

Per file you can:

- **Update** (circular-arrow button): re-pick a newer export. The calendar
  keeps its identity, so its enabled state and already-synced events stay
  attached and are refreshed on the next sync.
- **Remove** (trash button): delete the stored copy; the calendar and its
  events disappear on the next sync.

The calendar's display name comes from the file's `X-WR-CALNAME` property
when present, otherwise from the file name.

## What is supported

- **Events** (`VEVENT`): title, description, location, URL, organizer,
  attendees (with participation status/role), and event status
  (confirmed/tentative/cancelled).
- **Dates and times**: UTC (`...Z`), `TZID=`-qualified local times (IANA
  timezones), all-day `DATE` values, and `DURATION` when `DTEND` is missing.
  Floating times (no timezone at all) are interpreted in your machine's local
  timezone.
- **Recurring events**: `RRULE` is expanded inside the queried window
  (including `EXDATE` exclusions and `RDATE` additions), and single-instance
  overrides via `RECURRENCE-ID` (moved or cancelled occurrences) are honored.

## Known limitations

- An `.ics` file is a snapshot. Notare does **not** watch the original file
  or a subscription URL for changes — re-import via **Update** when the
  source calendar changes. (Subscribing to `webcal://`/HTTP feeds is a
  possible future addition.)
- Recurrence expansion uses the [`rrule`](https://crates.io/crates/rrule)
  crate. Exotic rules it cannot parse — and recurring **all-day** series with
  `EXDATE`/`RDATE` given as `DATE` values — degrade gracefully: the first
  instance is shown and a warning is logged.
- Recurring all-day series are expanded from UTC midnight, so an occurrence
  may appear on the neighbouring day for extreme timezone offsets.
- `VTIMEZONE` definitions embedded in the file are ignored; `TZID` values
  must be IANA names (e.g. `Europe/Berlin`). Non-IANA/Windows timezone ids
  fall back to single-instance behavior for recurring events.
- Files are read-only sources: events cannot be created or edited in an
  imported calendar.
- A malformed file is rejected at import with an error; individually broken
  events inside an otherwise valid file are skipped with a logged warning.
