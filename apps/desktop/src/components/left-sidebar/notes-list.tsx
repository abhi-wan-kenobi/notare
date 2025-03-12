import { useQuery } from "@tanstack/react-query";
import { isFuture } from "date-fns";
import { CalendarIcon } from "lucide-react";

import { useHypr, useSession } from "@/contexts";
import { formatDateHeader, getSortedDates, groupSessionsByDate } from "@/lib/date";

import { commands as dbCommands, type Session } from "@hypr/plugin-db";
import { NoteItem } from "./note-item";
import { EventItem } from "./event-item";

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
                <NoteItem
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
