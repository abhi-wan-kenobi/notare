import { Trans } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CalendarIcon, VideoIcon } from "lucide-react";

import { useHypr } from "@/contexts";
import { commands as appleCalendarCommands } from "@hypr/plugin-apple-calendar";
import { commands as dbCommands } from "@hypr/plugin-db";
import { commands as miscCommands } from "@hypr/plugin-misc";
import { Button } from "@hypr/ui/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/ui/lib/utils";
import { useSession } from "@hypr/utils/contexts";
import { formatRelativeWithDay } from "@hypr/utils/datetime";

interface EventChipProps {
  sessionId: string;
}

export function EventChip({ sessionId }: EventChipProps) {
  const { onboardingSessionId } = useHypr();

  const { sessionCreatedAt } = useSession(sessionId, (s) => ({
    sessionCreatedAt: s.session.created_at,
  }));

  const event = useQuery({
    queryKey: ["event", sessionId],
    queryFn: async () => {
      const event = await dbCommands.sessionGetEvent(sessionId);
      if (!event) {
        return null;
      }

      const meetingLink = await miscCommands.parseMeetingLink(event.note);
      return { ...event, meetingLink };
    },
  });

  const calendar = useQuery({
    enabled: !!event.data?.calendar_id,
    queryKey: ["calendar", event.data?.calendar_id],
    queryFn: async () => {
      const id = event.data?.calendar_id!;
      return dbCommands.getCalendar(id);
    },
  });

  const handleClickCalendar = () => {
    if (calendar.data) {
      if (calendar.data.platform === "Apple") {
        appleCalendarCommands.openCalendar();
      }
    }
  };

  const date = event.data?.start_date ?? sessionCreatedAt;

  return (
    <Popover>
      <PopoverTrigger
        disabled={!event.data || onboardingSessionId === sessionId}
      >
        <div
          className={cn(
            "flex flex-row items-center gap-2 rounded-md px-2 py-1.5",
            event.data
              && onboardingSessionId !== sessionId
              && "hover:bg-neutral-100",
          )}
        >
          {event.data?.meetingLink ? <VideoIcon size={14} /> : <CalendarIcon size={14} />}
          <p className="text-xs">{formatRelativeWithDay(date)}</p>
        </div>
      </PopoverTrigger>
      <PopoverContent align="start" className="shadow-lg w-72">
        <div className="flex flex-col gap-2">
          <div className="font-semibold">{event.data?.name}</div>
          <div className="text-sm text-neutral-600 whitespace-pre-wrap break-words max-h-24 overflow-y-auto">
            {event.data?.note}
          </div>
          {event.data?.meetingLink && (
            <Button
              variant="outline"
              className="flex items-center gap-2 text-xs overflow-hidden text-ellipsis whitespace-nowrap"
              onClick={() => {
                const meetingLink = event.data?.meetingLink;
                if (typeof meetingLink === "string") {
                  openUrl(meetingLink);
                }
              }}
            >
              <VideoIcon size={14} />
              <span className="truncate">Join meeting</span>
            </Button>
          )}
          <Button variant="outline" onClick={handleClickCalendar}>
            <Trans>View in calendar</Trans>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
