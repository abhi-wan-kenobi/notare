import { useMemo } from "react";

import { getStoredSettingValues, updateSettingValue } from "~/settings/queries";
import { resolveConfigValue, useConfigValue } from "~/shared/config";

export function parseHiddenCalendarProviders(value: unknown): string[] {
  let parsed: unknown = value;
  if (typeof value === "string") {
    try {
      parsed = JSON.parse(value);
    } catch {
      return [];
    }
  }
  if (!Array.isArray(parsed)) return [];
  return [
    ...new Set(
      parsed.filter((entry): entry is string => typeof entry === "string"),
    ),
  ].sort();
}

// Providers are passed to SQLite as ONE JSON-array parameter and expanded with
// `json_each(?)`, so the SQL text stays static and the parameter list stays one
// entry long however many providers are hidden.
export function hiddenProvidersParam(hidden: readonly string[]): string {
  return JSON.stringify(hidden);
}

// For queries that already LEFT JOIN `calendars AS calendar` (a missing or
// deleted calendar has no provider, so its events stay visible).
export const EXCLUDE_HIDDEN_PROVIDER_JOINED_SQL = `(calendar.provider IS NULL OR calendar.provider NOT IN (SELECT value FROM json_each(?)))`;

// For queries over `events` alone; the matching `?` is the hidden-providers
// JSON param.
export function excludeHiddenProviderEventsSql(eventAlias = "events"): string {
  return `NOT EXISTS (
        SELECT 1
        FROM calendars AS hidden_calendar
        WHERE hidden_calendar.id = ${eventAlias}.calendar_id
          AND hidden_calendar.deleted_at IS NULL
          AND hidden_calendar.provider IN (SELECT value FROM json_each(?))
      )`;
}

export function useHiddenCalendarProviders(): string[] {
  const raw = useConfigValue("calendar_hidden_providers");
  return useMemo(() => parseHiddenCalendarProviders(raw), [raw]);
}

export async function getHiddenCalendarProviders(): Promise<string[]> {
  const stored = await getStoredSettingValues();
  return parseHiddenCalendarProviders(
    resolveConfigValue("calendar_hidden_providers", stored),
  );
}

export function setCalendarProviderHidden(
  provider: string,
  hidden: boolean,
): Promise<unknown> {
  return updateSettingValue("calendar_hidden_providers", (current) => {
    const next = new Set(parseHiddenCalendarProviders(current));
    if (hidden) next.add(provider);
    else next.delete(provider);
    return hiddenProvidersParam([...next].sort());
  });
}
