-- Only the internal session grouping changes. Remote/provider recurrence ids
-- remain opaque and untouched in events and embedded event snapshots.
-- Resolve a real event first; never resolve an external id across calendars.
-- Unlinked/manual history without scoped event metadata keeps its grouping.
WITH snapshots AS (
  SELECT sessions.*,
    CASE WHEN json_valid(event_json) THEN event_json ELSE '{}' END AS snapshot
  FROM sessions
), scoped AS (
  SELECT s.id, COALESCE(
    (SELECT 'calendar-series:' || json_array(e.provider, e.calendar_id, e.recurrence_series_id)
     FROM events e
     WHERE s.event_id <> '' AND e.id = s.event_id
       AND e.calendar_id <> '' AND e.recurrence_series_id <> ''
     LIMIT 1),
    (SELECT 'calendar-series:' || json_array(e.provider, e.calendar_id, e.recurrence_series_id)
     FROM events e
     WHERE s.external_event_id <> '' AND e.tracking_id_event = s.external_event_id
       AND e.calendar_id = json_extract(s.snapshot, '$.calendar_id')
       AND e.provider = COALESCE(NULLIF(s.external_provider, ''),
         NULLIF(json_extract(s.snapshot, '$.provider'), ''),
         (SELECT provider FROM calendars WHERE id = json_extract(s.snapshot, '$.calendar_id')), '')
       AND e.calendar_id <> '' AND e.recurrence_series_id <> ''
     ORDER BY e.started_at DESC, e.id LIMIT 1),
    CASE WHEN COALESCE(json_extract(s.snapshot, '$.calendar_id'), '') <> ''
      AND COALESCE(json_extract(s.snapshot, '$.recurrence_series_id'), '') <> ''
    THEN 'calendar-series:' || json_array(
      COALESCE(NULLIF(json_extract(s.snapshot, '$.provider'), ''),
        NULLIF(s.external_provider, ''),
        (SELECT provider FROM calendars WHERE id = json_extract(s.snapshot, '$.calendar_id')),
        ''),
      json_extract(s.snapshot, '$.calendar_id'),
      json_extract(s.snapshot, '$.recurrence_series_id'))
    END
  ) AS series_id
  FROM snapshots s
)
UPDATE sessions SET series_id = scoped.series_id
FROM scoped
WHERE sessions.id = scoped.id AND scoped.series_id IS NOT NULL
  AND sessions.series_id <> scoped.series_id;
