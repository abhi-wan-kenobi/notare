import { isToday, isYesterday, startOfDay } from "@hypr/utils";

import type { DictationHistoryEntry } from "~/dictation/history";

/**
 * What a day bucket's header should show. Kept as data (not a translated
 * string) so this module stays pure/testable - the component maps `kind` to
 * `t\`Today\`` / `t\`Yesterday\`` / a formatted absolute date.
 */
export type DayBucketKind =
  | { kind: "today" }
  | { kind: "yesterday" }
  | { kind: "date"; date: Date }
  /** `createdAt` didn't parse as a date - still shown, not silently dropped. */
  | { kind: "unknown" };

export interface DayBucket {
  /** Stable React key: an ISO day string, or "unknown". */
  key: string;
  bucket: DayBucketKind;
  entries: DictationHistoryEntry[];
}

/**
 * Groups history entries (assumed newest-first, per
 * `listDictationHistory`'s ordering) into contiguous day buckets. Does not
 * re-sort - a run of same-day entries is expected to already be adjacent.
 */
export function groupEntriesByDay(
  entries: DictationHistoryEntry[],
): DayBucket[] {
  const buckets: DayBucket[] = [];

  for (const entry of entries) {
    const parsed = new Date(entry.createdAt);
    const dayStart = Number.isNaN(parsed.getTime())
      ? null
      : startOfDay(parsed);
    const key = dayStart ? dayStart.toISOString() : "unknown";

    const last = buckets[buckets.length - 1];
    if (last && last.key === key) {
      last.entries.push(entry);
      continue;
    }

    buckets.push({
      key,
      bucket: dayStart ? dayBucketKindFor(dayStart) : { kind: "unknown" },
      entries: [entry],
    });
  }

  return buckets;
}

function dayBucketKindFor(dayStart: Date): DayBucketKind {
  if (isToday(dayStart)) {
    return { kind: "today" };
  }
  if (isYesterday(dayStart)) {
    return { kind: "yesterday" };
  }
  return { kind: "date", date: dayStart };
}
