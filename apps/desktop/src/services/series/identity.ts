type RecurringEventIdentity = {
  provider?: string | null;
  calendar_id?: string | null;
  recurrence_series_id?: string | null;
};

// Provider recurrence ids are opaque and only meaningful inside a calendar.
// Keep the provider's raw id in events/event_json; only sessions.series_id uses
// this encoded tuple. Unlinked/manual series keep their existing identifiers.
export function seriesIdForEvent(event: RecurringEventIdentity): string {
  if (!event.calendar_id || !event.recurrence_series_id) return "";
  return `calendar-series:${JSON.stringify([
    event.provider ?? "",
    event.calendar_id,
    event.recurrence_series_id,
  ])}`;
}

export const EVENT_SERIES_ID_SQL = `CASE
  WHEN calendar_id <> '' AND recurrence_series_id <> ''
  THEN 'calendar-series:' || json_array(COALESCE(provider, ''), calendar_id, recurrence_series_id)
  ELSE '' END`;
