import { CalendarIcon } from "lucide-react";

import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";
import { Button } from "@hypr/ui/components/ui/button";

interface CalendarEvent {
  id: string;
  title: string;
  start_time: string;
  end_time: string;
  location?: string;
}

interface CalendarEventSheetProps {
  open: boolean;
  onClose: () => void;
  event: CalendarEvent;
  onViewInCalendar: () => void;
  formatEventTime: (startTime: string, endTime: string) => string;
}

export function CalendarEventSheet({
  open,
  onClose,
  event,
  onViewInCalendar,
  formatEventTime,
}: CalendarEventSheetProps) {
  return (
    <BottomSheet open={open} onClose={onClose}>
      <BottomSheetContent className="bg-white">
        <div className="p-4 space-y-4">
          <h3 className="text-lg font-medium mb-2">Event Details</h3>

          <div className="space-y-3">
            <div className="space-y-1">
              <h4 className="font-medium">{event.title}</h4>
              <p className="text-sm text-neutral-500">
                {new Date(event.start_time).toLocaleDateString("en-US", {
                  weekday: "long",
                  month: "long",
                  day: "numeric",
                  year: "numeric",
                })}
              </p>
              <p className="text-sm text-neutral-500">
                {formatEventTime(event.start_time, event.end_time)}
              </p>
              {event.location && <p className="text-sm text-neutral-500">{event.location}</p>}
            </div>

            <Button
              className="w-full mt-4"
              variant="outline"
              onClick={onViewInCalendar}
            >
              <CalendarIcon className="mr-2 size-4" />
              View in Calendar
            </Button>
          </div>
        </div>
      </BottomSheetContent>
    </BottomSheet>
  );
}
