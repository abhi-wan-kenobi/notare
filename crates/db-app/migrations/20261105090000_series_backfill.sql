ALTER TABLE action_items ADD COLUMN carried_from_session_id TEXT;

-- Backfill sessions.series_id from the linked recurring event. Each lookup is
-- guarded so an empty key on the session side can never match an empty key
-- on the events side, and ordered so the result is deterministic.
UPDATE sessions
SET series_id = COALESCE(
  NULLIF(CASE WHEN json_valid(event_json) = 1 THEN json_extract(event_json, '$.recurrence_series_id') END, ''),
  NULLIF((
    SELECT recurrence_series_id
    FROM events
    WHERE sessions.event_id <> ''
      AND events.id = sessions.event_id
      AND events.deleted_at IS NULL
    ORDER BY events.started_at DESC, events.id
    LIMIT 1
  ), ''),
  NULLIF((
    SELECT recurrence_series_id
    FROM events
    WHERE sessions.external_event_id <> ''
      AND events.tracking_id_event = sessions.external_event_id
      AND events.deleted_at IS NULL
    ORDER BY events.started_at DESC, events.id
    LIMIT 1
  ), ''),
  series_id
)
WHERE (series_id IS NULL OR series_id = '')
  AND deleted_at IS NULL;
