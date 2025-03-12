import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useFlow, useLoaderData } from "@stackflow/react/future";
import { AudioLinesIcon, CalendarIcon, MicIcon, Settings, SquarePenIcon } from "lucide-react";

import { EventItem } from "../components/event-item";
import { NoteItem } from "../components/note-item";
import { mockEvents, mockSessions } from "../mock";
import { formatDateHeader, getSortedDates, groupNotesByDate } from "../utils/date";

import { type Session } from "@hypr/plugin-db";
import { Button } from "@hypr/ui/components/ui/button";

export function homeActivityLoader({}: ActivityLoaderArgs<"HomeActivity">) {
  return {
    upcomingEvents: mockEvents,
    notes: mockSessions,
  };
}

export const HomeActivity: ActivityComponentType<"HomeActivity"> = () => {
  const { upcomingEvents, notes } = useLoaderData<typeof homeActivityLoader>();

  const { push } = useFlow();

  const groupedSessions = groupNotesByDate(notes ?? []);
  const sortedDates = getSortedDates(groupedSessions);

  const handleClickNote = (id: string) => {
    push("NoteActivity", { id });
  };

  const handleClickNew = () => {
    push("NoteActivity", { id: "new" });
  };

  const handleClickSettings = () => {
    push("SettingsActivity", {});
  };

  const RightButton = () => (
    <button onClick={handleClickSettings}>
      <Settings size={20} />
    </button>
  );

  return (
    <AppScreen
      appBar={{
        title: "All Notes",
        renderRight: RightButton,
      }}
    >
      <div className="relative flex h-full flex-col">
        <div className="flex-1 overflow-y-auto px-4 pb-20">
          {upcomingEvents && upcomingEvents.length > 0 && (
            <section className="mt-4 mb-6">
              <h2 className="font-medium text-neutral-600 mb-3 flex items-center gap-2">
                <CalendarIcon className="size-4" />
                <strong>Upcoming</strong>
              </h2>

              <div className="space-y-2">
                {upcomingEvents.map((event) => (
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
                    <NoteItem
                      key={session.id}
                      session={session}
                      onSelect={() => handleClickNote(session.id)}
                    />
                  ))}
                </div>
              </section>
            );
          })}

          {notes && notes.length === 0 && (
            <div className="flex flex-col justify-center items-center h-64">
              <p className="text-neutral-500 mb-4">No notes yet</p>
              <Button onClick={handleClickNew}>Create your first note</Button>
            </div>
          )}
        </div>

        <div
          className="absolute z-10 bottom-0 left-0 right-0 flex justify-center px-4 pb-4"
          onClick={(e) => e.stopPropagation()}
        >
          <Sheet>
            <SheetTrigger asChild>
              <Button className="w-full py-3 text-lg font-semibold">
                <SquarePenIcon size={20} />Create new note
              </Button>
            </SheetTrigger>
            <SheetContent className="flex gap-2 max-h-60">
              <Button className="aspect-square w-full flex-col gap-1" variant="outline">
                <AudioLinesIcon size={32} />
              </Button>
              <Button className="aspect-square w-full flex-col gap-1">
                <MicIcon size={32} />
              </Button>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    HomeActivity: {};
  }
}
