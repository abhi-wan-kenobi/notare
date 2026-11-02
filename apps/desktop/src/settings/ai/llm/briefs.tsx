import { Trans } from "@lingui/react/macro";
import { useId } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { Switch } from "@hypr/ui/components/ui/switch";

import { useTaskModel } from "~/services/llm-router";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

const LEAD_TIME_OPTIONS = [
  { value: "2", label: "2 hours" },
  { value: "4", label: "4 hours" },
  { value: "8", label: "8 hours" },
  { value: "12", label: "12 hours (default)" },
  { value: "24", label: "24 hours" },
] as const;

export function BriefsSettings() {
  const titleId = useId();
  const descId = useId();
  const leadTimeTitleId = useId();
  const leadTimeDescId = useId();

  const enabled = useConfigValue("pre_meeting_briefs_enabled");
  const leadTimeHours = useConfigValue("pre_meeting_briefs_lead_time_hours");
  const setEnabled = useSetSettingValue("pre_meeting_briefs_enabled");
  const setLeadTime = useSetSettingValue("pre_meeting_briefs_lead_time_hours");

  const { model, resolution } = useTaskModel("brief");
  const hasModel = resolution.status === "ok" && Boolean(model);

  const isChecked = hasModel ? (enabled ?? true) : false;
  const currentLeadTime = String(leadTimeHours ?? 12);

  return (
    <section className="flex flex-col gap-4">
      <h3 className="text-md font-sans font-semibold">
        <Trans>Pre-Meeting Briefs</Trans>
      </h3>

      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h4 id={titleId} className="mb-1 text-sm font-medium">
            <Trans>Generate pre-meeting briefs</Trans>
          </h4>
          <p id={descId} className="text-muted-foreground text-xs">
            <Trans>
              Automatically synthesize attendee background, previous meeting
              outcomes, open action items, and suggested questions before
              upcoming meetings.
            </Trans>
          </p>
        </div>
        <Switch
          checked={isChecked}
          disabled={!hasModel}
          onCheckedChange={setEnabled}
          aria-labelledby={titleId}
          aria-describedby={descId}
        />
      </div>

      {!hasModel && (
        <p className="text-muted-foreground text-xs italic">
          <Trans>
            Configure a local or cloud model above to enable automatic
            pre-meeting briefs.
          </Trans>
        </p>
      )}

      {isChecked && (
        <div className="grid grid-cols-[minmax(0,1fr)_12rem] items-center gap-3 pt-1">
          <div className="flex min-w-0 flex-col">
            <span id={leadTimeTitleId} className="truncate text-sm font-medium">
              <Trans>Preparation window</Trans>
            </span>
            <span id={leadTimeDescId} className="text-muted-foreground text-xs">
              <Trans>
                How far in advance to generate briefs before events start.
              </Trans>
            </span>
          </div>
          <Select
            value={currentLeadTime}
            onValueChange={(val) => setLeadTime(Number.parseInt(val, 10))}
          >
            <SelectTrigger
              className="bg-card h-9 w-full shadow-none focus:ring-0"
              aria-labelledby={leadTimeTitleId}
              aria-describedby={leadTimeDescId}
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {LEAD_TIME_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
    </section>
  );
}
