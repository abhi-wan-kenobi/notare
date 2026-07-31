import { Trans, useLingui } from "@lingui/react/macro";
import { useId } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";

/**
 * How long Snippets/dictation history is kept before being pruned by age
 * (`pruneDictationHistoryByAge` in `dictation/history.ts`). "off" keeps
 * everything (subject only to the existing rolling count cap). Pinned
 * entries are always exempt from age-based pruning, regardless of this
 * setting - see the copy below.
 */
export const DICTATION_HISTORY_RETENTION_OPTIONS = [
  "off",
  "7d",
  "30d",
  "90d",
] as const;

export type DictationHistoryRetention =
  (typeof DICTATION_HISTORY_RETENTION_OPTIONS)[number];

export function normalizeHistoryRetention(
  raw: string | undefined,
): DictationHistoryRetention {
  return (DICTATION_HISTORY_RETENTION_OPTIONS as readonly string[]).includes(
    raw ?? "",
  )
    ? (raw as DictationHistoryRetention)
    : "off";
}

/**
 * Retention picker for the History section: same Select-row pattern as
 * `settings/general/storage`'s audio retention row. Enforcement is
 * age-based (day-to-day, fired from the Snippets page load) rather than
 * synchronous with this setting change - see `dictation/history.ts`'s
 * `pruneDictationHistoryByAge`.
 */
export function HistoryRetentionRow({
  value,
  onChange,
}: {
  value: string;
  onChange: (next: string) => void;
}) {
  const { t } = useLingui();
  const titleId = useId();
  const descriptionId = useId();
  const normalized = normalizeHistoryRetention(value);
  const labelByValue: Record<DictationHistoryRetention, string> = {
    off: t`Keep everything`,
    "7d": t`7 days`,
    "30d": t`30 days`,
    "90d": t`90 days`,
  };

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
      <div className="flex min-w-0 flex-col">
        <span id={titleId} className="truncate text-sm font-medium">
          <Trans>Keep history for</Trans>
        </span>
        <span id={descriptionId} className="text-muted-foreground text-xs">
          <Trans>
            Pinned snippets are always kept, no matter this setting.
          </Trans>
        </span>
      </div>
      <Select value={normalized} onValueChange={onChange}>
        <SelectTrigger
          className="bg-card h-9 w-full shadow-none focus:ring-0"
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {DICTATION_HISTORY_RETENTION_OPTIONS.map((option) => (
            <SelectItem key={option} value={option}>
              {labelByValue[option]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
