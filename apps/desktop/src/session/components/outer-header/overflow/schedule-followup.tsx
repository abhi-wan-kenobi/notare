import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { CalendarPlusIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  addMinutesIso,
  DEFAULT_DURATION_MINUTES,
  datetimeLocalToIso,
  defaultFollowupAttendees,
  defaultFollowupDescription,
  defaultFollowupStart,
  defaultFollowupTitle,
  DURATION_OPTIONS_MINUTES,
  type FollowupAttendee,
  toDatetimeLocalValue,
} from "./schedule-followup-defaults";
import {
  submitFollowupEvent,
  useWritableCalendarTargets,
  type WritableCalendarTarget,
} from "./schedule-followup-queries";

import { useSessionActionItems } from "~/session/components/action-items/queries";
import { useSession, useSessionParticipants } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";

export function ScheduleFollowupMenuItem({
  onSelect,
}: {
  onSelect: () => void;
}) {
  const { data: targets, isLoading } = useWritableCalendarTargets();

  if (isLoading || !targets || targets.length === 0) {
    return null;
  }

  return (
    <DropdownMenuItem onClick={onSelect} className="cursor-pointer">
      <CalendarPlusIcon />
      <span>
        <Trans>Schedule follow-up</Trans>
      </span>
    </DropdownMenuItem>
  );
}

export function ScheduleFollowupDialog({
  sessionId,
  open,
  onOpenChange,
}: {
  sessionId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useLingui();
  const session = useSession(sessionId);
  const event = session ? getSessionEvent(session) : null;
  const participants = useSessionParticipants(sessionId);
  const { items: actionItems } = useSessionActionItems(sessionId);
  const { data: targets = [] } = useWritableCalendarTargets();

  const [title, setTitle] = useState("");
  const [startLocal, setStartLocal] = useState("");
  const [durationMinutes, setDurationMinutes] = useState<number>(
    DEFAULT_DURATION_MINUTES,
  );
  const [calendarKey, setCalendarKey] = useState("");
  const [attendees, setAttendees] = useState<FollowupAttendee[]>([]);
  const [description, setDescription] = useState("");
  // Tracks whether the user has touched title/attendees/description for the
  // current open cycle, so late-arriving session/participant/action-item
  // data (still loading when the dialog opened) can keep syncing in without
  // clobbering edits once the user starts typing.
  const userEditedRef = useRef(false);

  // Date/time and duration don't depend on session data, so they're set
  // once per dialog-open transition.
  useEffect(() => {
    if (!open) return;
    setStartLocal(toDatetimeLocalValue(defaultFollowupStart()));
    setDurationMinutes(DEFAULT_DURATION_MINUTES);
    userEditedRef.current = false;
  }, [open, sessionId]);

  // Title/attendees/description depend on session/participants/action-item
  // queries, which may still be loading when the dialog opens. Keep syncing
  // defaults from them until the user edits a field.
  useEffect(() => {
    if (!open || userEditedRef.current) return;
    const meetingTitle = event?.title || session?.title || "";
    setTitle(defaultFollowupTitle(meetingTitle));
    setAttendees(defaultFollowupAttendees(participants));
    setDescription(defaultFollowupDescription(actionItems));
  }, [open, session, event, participants, actionItems]);

  useEffect(() => {
    if (!open) return;
    if (calendarKey && targets.some((target) => target.key === calendarKey)) {
      return;
    }
    setCalendarKey(targets[0]?.key ?? "");
  }, [open, targets, calendarKey]);

  const selectedTarget = useMemo(
    () => targets.find((target) => target.key === calendarKey) ?? null,
    [targets, calendarKey],
  );

  const createEventMutation = useMutation({
    mutationFn: async () => {
      if (!selectedTarget) {
        throw new Error(t`Choose a calendar`);
      }
      const startIso = datetimeLocalToIso(startLocal);
      if (!startIso) {
        throw new Error(t`Enter a valid date and time`);
      }
      const endIso = addMinutesIso(startIso, durationMinutes);
      if (!endIso) {
        throw new Error(t`Enter a valid date and time`);
      }
      return submitFollowupEvent({
        target: selectedTarget,
        title: title.trim() || defaultFollowupTitle(null),
        startIso,
        endIso,
        description,
        attendees,
      });
    },
    onSuccess: () => {
      sonnerToast.success(t`Follow-up scheduled`);
      onOpenChange(false);
    },
    onError: (error: Error) => {
      sonnerToast.error(error.message);
    },
  });

  const removeAttendee = (id: string) => {
    userEditedRef.current = true;
    setAttendees((current) => current.filter((a) => a.id !== id));
  };

  const canSubmit =
    Boolean(selectedTarget) &&
    Boolean(title.trim()) &&
    Boolean(datetimeLocalToIso(startLocal));

  const waitingLabel = selectedTarget
    ? t`Waiting for ${selectedTarget.provider === "google" ? "Google" : "Outlook"}… finish sign-in in your browser if prompted`
    : t`Creating event…`;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            createEventMutation.mutate();
          }}
          className="flex flex-col gap-4"
        >
          <DialogHeader>
            <DialogTitle>
              <Trans>Schedule follow-up</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>Create a calendar event for the next meeting.</Trans>
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-foreground text-xs font-medium">
                <Trans>Title</Trans>
              </label>
              <Input
                value={title}
                onChange={(e) => {
                  userEditedRef.current = true;
                  setTitle(e.target.value);
                }}
                className="text-xs"
                autoFocus
              />
            </div>

            <div className="flex gap-3">
              <div className="flex flex-1 flex-col gap-1.5">
                <label className="text-foreground text-xs font-medium">
                  <Trans>Date &amp; time</Trans>
                </label>
                <Input
                  type="datetime-local"
                  value={startLocal}
                  onChange={(e) => setStartLocal(e.target.value)}
                  className="text-xs"
                />
              </div>

              <div className="flex w-28 flex-col gap-1.5">
                <label className="text-foreground text-xs font-medium">
                  <Trans>Duration</Trans>
                </label>
                <Select
                  value={String(durationMinutes)}
                  onValueChange={(value) => setDurationMinutes(Number(value))}
                >
                  <SelectTrigger className="text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {DURATION_OPTIONS_MINUTES.map((minutes) => (
                      <SelectItem key={minutes} value={String(minutes)}>
                        <Trans>{minutes} min</Trans>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-foreground text-xs font-medium">
                <Trans>Calendar</Trans>
              </label>
              <Select value={calendarKey} onValueChange={setCalendarKey}>
                <SelectTrigger className="text-xs">
                  <SelectValue placeholder={t`Choose a calendar`} />
                </SelectTrigger>
                <SelectContent>
                  {targets.map((target) => (
                    <SelectItem key={target.key} value={target.key}>
                      {target.calendarTitle} — {target.accountEmail}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {attendees.length > 0 && (
              <div className="flex flex-col gap-1.5">
                <label className="text-foreground text-xs font-medium">
                  <Trans>Attendees</Trans>
                </label>
                <div className="flex flex-wrap gap-1.5">
                  {attendees.map((attendee) => (
                    <Badge
                      key={attendee.id}
                      variant="secondary"
                      className="flex items-center gap-1 px-2 py-0.5 text-xs"
                    >
                      {attendee.name}
                      <button
                        type="button"
                        onClick={() => removeAttendee(attendee.id)}
                        aria-label={t`Remove ${attendee.name}`}
                        className="hover:text-destructive cursor-pointer"
                      >
                        <XIcon size={12} />
                      </button>
                    </Badge>
                  ))}
                </div>
              </div>
            )}

            <div className="flex flex-col gap-1.5">
              <label className="text-foreground text-xs font-medium">
                <Trans>Description</Trans>
              </label>
              <textarea
                value={description}
                onChange={(e) => {
                  userEditedRef.current = true;
                  setDescription(e.target.value);
                }}
                rows={4}
                className="border-input placeholder:text-muted-foreground focus-visible:ring-ring flex w-full rounded-md border bg-transparent px-3 py-2 text-xs shadow-xs focus-visible:ring-1 focus-visible:outline-hidden"
              />
            </div>
          </div>

          {createEventMutation.isError && (
            <p className="text-xs text-red-600">
              {createEventMutation.error.message}
            </p>
          )}

          <DialogFooter className="gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => onOpenChange(false)}
              disabled={createEventMutation.isPending}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={!canSubmit || createEventMutation.isPending}
            >
              {createEventMutation.isPending ? waitingLabel : t`Schedule`}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export type { WritableCalendarTarget };
