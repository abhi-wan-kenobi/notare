import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useFlow } from "@stackflow/react/future";
import { useQuery } from "@tanstack/react-query";
import { format, isFuture } from "date-fns";
import { CalendarIcon, Settings } from "lucide-react";
import { useHypr } from "../contexts/hypr";
import { formatDateHeader, formatRemainingTime, getSortedDates, groupSessionsByDate } from "../utils/date";

import { commands as dbCommands, type Event, type Session } from "@hypr/plugin-db";
import { Avatar, AvatarFallback, AvatarImage } from "@hypr/ui/components/ui/avatar";
import { Button } from "@hypr/ui/components/ui/button";

export function homeActivityLoader({}: ActivityLoaderArgs<"HomeActivity">) {
  return {};
}

export const HomeActivity: ActivityComponentType<"HomeActivity"> = () => {
  const { userId } = useHypr();
  const { push } = useFlow();

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

  const handleClickNote = (id: string) => {
    push("NoteActivity", { id });
  };

  const handleClickNew = () => {
    push("NoteActivity", { id: "new" });
  };

  const handleClickProfile = () => {
    push("ProfileActivity", {});
  };

  const handleClickSettings = () => {
    push("SettingsActivity", {});
  };

  const LeftButton = () => (
    <button onClick={handleClickProfile}>
      <Avatar className="size-7 border text-sm font-medium">
        <AvatarImage src="/path-to-user-profile-image.jpg" alt="User profile" />
        <AvatarFallback>J</AvatarFallback>
      </Avatar>
    </button>
  );

  const RightButton = () => (
    <button onClick={handleClickSettings}>
      <Settings size={20} />
    </button>
  );

  return (
    <AppScreen
      appBar={{
        title: "All Notes",
        renderLeft: LeftButton,
        renderRight: RightButton,
      }}
    >
      <div className="relative flex h-full flex-col">
        <div className="flex-1 overflow-y-auto px-4 pb-20">
          {events.data && events.data.length > 0 && (
            <section className="mt-4 mb-6">
              <h2 className="font-medium text-neutral-600 mb-3 flex items-center gap-2">
                <CalendarIcon className="size-4" />
                <strong>Upcoming</strong>
              </h2>

              <div className="space-y-2">
                {events.data.map((event) => (
                  <EventItem
                    key={event.id}
                    event={event}
                    onSelect={(sessionId) => handleClickNote(sessionId)}
                  />
                ))}
              </div>
            </section>
          )}

          {sortedDates.map((dateKey) => {
            const { date, sessions } = groupedSessions[dateKey];

            return (
              <section key={dateKey} className="mb-6">
                <h2 className="font-bold text-neutral-600 mb-3">
                  {formatDateHeader(date)}
                </h2>

                <div className="space-y-2">
                  {sessions.map((session: Session) => (
                    <SessionItem
                      key={session.id}
                      session={session}
                      onSelect={() => handleClickNote(session.id)}
                    />
                  ))}
                </div>
              </section>
            );
          })}

          {sessions.isLoading && (
            <div className="flex justify-center items-center h-32">
              <p className="text-neutral-500">Loading notes...</p>
            </div>
          )}

          {sessions.data && sessions.data.length === 0 && !sessions.isLoading && (
            <div className="flex flex-col justify-center items-center h-64">
              <p className="text-neutral-500 mb-4">No notes yet</p>
              <Button onClick={handleClickNew}>Create your first note</Button>
            </div>
          )}
        </div>

        <div className="absolute z-10 bottom-0 left-0 right-0 flex justify-center p-4 bg-white border-t border-gray-200">
          <Button className="w-full" onClick={handleClickNew}>New note</Button>
        </div>
      </div>
    </AppScreen>
  );
};

function EventItem({ event, onSelect }: { event: Event; onSelect: (sessionId: string) => void }) {
  const session = useQuery({
    queryKey: ["event-session", event.id],
    queryFn: async () => dbCommands.getSession({ calendarEventId: event.id }),
  });

  const handleClick = () => {
    if (session.data) {
      onSelect(session.data.id);
    }
  };

  return (
    <button
      onClick={handleClick}
      className="w-full text-left group flex items-start gap-3 py-3 hover:bg-neutral-100 rounded-lg px-3 border border-neutral-200"
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
  onSelect,
}: {
  session: Session;
  onSelect: () => void;
}) {
  const sessionDate = new Date(session.created_at);

  return (
    <button
      onClick={onSelect}
      className="hover:bg-neutral-100 group flex items-start gap-3 py-3 w-full text-left transition-all rounded-lg px-3 border border-neutral-200"
    >
      <div className="flex flex-col items-start gap-1">
        <div className="font-medium text-sm line-clamp-1">
          {session.title || "Untitled"}
        </div>
        <div className="flex items-center gap-2 text-xs text-neutral-500">
          <span>{format(sessionDate, "M/d/yy")}</span>
        </div>
      </div>
    </button>
  );
}

declare module "@stackflow/config" {
  interface Register {
    HomeActivity: {};
  }
}
