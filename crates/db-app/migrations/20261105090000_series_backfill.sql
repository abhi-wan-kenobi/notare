ALTER TABLE action_items ADD COLUMN carried_from_session_id TEXT;

UPDATE sessions
SET series_id = COALESCE(
  NULLIF(CASE WHEN json_valid(event_json) = 1 THEN json_extract(event_json, '$.recurrence_series_id') END, ''),
  NULLIF((
    SELECT recurrence_series_id
    FROM events
    WHERE events.id = sessions.event_id
      AND events.deleted_at IS NULL
    LIMIT 1
  ), ''),
  NULLIF((
    SELECT recurrence_series_id
    FROM events
    WHERE events.tracking_id_event = sessions.external_event_id
      AND events.deleted_at IS NULL
    LIMIT 1
  ), ''),
  series_id
)
WHERE (series_id IS NULL OR series_id = '')
  AND deleted_at IS NULL;
