import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { format, isFuture } from "date-fns";
import { CalendarIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { useHypr, useSession } from "@/contexts";
import { formatDateHeader, formatRemainingTime, getSortedDates, groupSessionsByDate } from "@/lib/date";
import { commands as windowsCommands } from "@hypr/plugin-windows";

import { commands as dbCommands, type Event, type Session } from "@hypr/plugin-db";
import { cn } from "@hypr/ui/lib/utils";

export default function NotesList() {
  const { userId } = useHypr();
  const currentSession = useSession((s) => s.session);

  const events = useQuery({
    queryKey: ["events"],
    queryFn: async () => {
      const events = await dbCommands.listEvents(userId);
      const upcomingEvents = events.filter((event) => {
        return isFuture(new Date(event.start_date));
      });

      return upcomingEvents;
    },
  });

  const sessions = useQuery({
    queryKey: ["sessions"],
    queryFn: () => dbCommands.listSessions(null),
  });

  const groupedSessions = groupSessionsByDate(sessions.data ?? []);
  const sortedDates = getSortedDates(groupedSessions);

  return (
    <nav className="h-full overflow-y-auto space-y-6 px-3 pb-4">
      {events.data && events.data.length > 0 && (
        <section>
          <h2 className="font-medium text-neutral-600 mb-2 flex items-center gap-2">
            <CalendarIcon className="size-4" />
            <strong>Upcoming</strong>
          </h2>

          <div>
            {events.data.map((event) => <EventItem key={event.id} event={event} />)}
          </div>
        </section>
      )}

      {sortedDates.map((dateKey) => {
        const { date, sessions } = groupedSessions[dateKey];

        return (
          <section key={dateKey}>
            <h2 className="font-bold text-neutral-600 mb-2">
              {formatDateHeader(date)}
            </h2>

            <div>
              {sessions.map((session: Session) => (
                <SessionItem
                  key={session.id}
                  session={session}
                  isActive={session.id === currentSession?.id}
                />
              ))}
            </div>
          </section>
        );
      })}
    </nav>
  );
}
function EventItem({ event }: { event: Event }) {
  const navigate = useNavigate();

  const session = useQuery({
    queryKey: ["event-session", event.id],
    queryFn: async () => dbCommands.getSession({ calendarEventId: event.id }),
  });

  const handleClick = () => {
    if (session.data) {
      navigate({
        to: "/app/note/$id",
        params: { id: session.data.id },
      });
    }
  };

  return (
    <button
      onClick={handleClick}
      className="w-full text-left group flex items-start gap-3 py-2 hover:bg-neutral-100 rounded px-2"
    >
      <div className="flex flex-col items-start gap-1">
        <div className="font-medium text-sm line-clamp-1">{event.name}</div>
        <div className="flex items-center gap-2 text-xs text-neutral-500 line-clamp-1">
          <span>{formatRemainingTime(new Date(event.start_date))}</span>
        </div>
      </div>
    </button>
  );
}

function SessionItem({
  session,
  isActive,
}: {
  session: Session;
  isActive: boolean;
}) {
  const navigate = useNavigate();
  const currentSession = useSession((s) => s.session);
  const sessionDate = new Date(session.created_at);

  const [clicks, setClicks] = useState(0);

  useEffect(() => {
    let singleClickTimer: ReturnType<typeof setTimeout>;
    if (clicks === 1) {
      singleClickTimer = setTimeout(() => {
        handleSingleClick();
        setClicks(0);
      }, 150);
    } else if (clicks === 2) {
      handleDoubleClick();
      setClicks(0);
    }
    return () => clearTimeout(singleClickTimer);
  }, [clicks]);

  const handleClick = () => {
    setClicks((c) => c + 1);
  };

  const handleSingleClick = () => {
    navigate({
      to: "/app/note/$id",
      params: { id: session.id },
    });
  };

  const handleDoubleClick = () => {
    windowsCommands.windowShow({ note: session.id });
  };

  return (
    <button
      onClick={handleClick}
      disabled={isActive}
      className={cn(
        "hover:bg-neutral-200 group flex items-start gap-3 py-2 w-full text-left transition-all rounded px-2",
        isActive && "bg-neutral-200",
      )}
    >
      <div className="flex flex-col items-start gap-1">
        <div className="font-medium text-sm max-w-[180px] truncate">
          {isActive
            ? currentSession?.title || "Untitled"
            : session.title || "Untitled"}
        </div>
        <div className="flex items-center gap-2 text-xs text-neutral-500">
          <span>{format(sessionDate, "M/d/yy")}</span>
        </div>
      </div>
    </button>
  );
}
