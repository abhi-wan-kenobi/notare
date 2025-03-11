import type { Session } from "../../client";
import { Users2Icon, CalendarIcon } from "lucide-react";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";

interface SessionInfoProps {
  session: Session;
}

export function SessionInfo({ session }: SessionInfoProps) {
  const hasParticipants =
    session.conversations.length > 0 &&
    session.conversations.some((conv) => conv.diarizations.length > 0);

  const participantsCount = hasParticipants
    ? session.conversations.flatMap((conv) => conv.diarizations).length
    : 0;

  const currentDate = new Date().toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  const uniqueParticipants = hasParticipants
    ? Array.from(
        new Set(
          session.conversations.flatMap((conv) =>
            conv.diarizations.map((d) => d.speaker),
          ),
        ),
      )
    : [];

  return (
    <div className="mx-auto max-w-2xl w-full flex flex-col px-4 py-8 overflow-x-auto scrollbar-none">
      <h2 className="text-xl font-medium text-neutral-800 mb-2">
        {session.title || "Untitled"}
      </h2>

      <div className="-mx-1.5 flex flex-row items-center whitespace-nowrap">
        <div className="flex flex-row items-center gap-2 rounded-md px-2 py-1.5 text-xs">
          <CalendarIcon size={14} />
          <span>{currentDate}</span>
        </div>

        {hasParticipants && (
          <div className="flex items-center">
            <div className="border-l border-neutral-200 h-4 mx-2"></div>
            <Popover>
              <PopoverTrigger className="flex flex-row items-center gap-2 rounded-md px-2 py-1.5 hover:bg-neutral-100 text-xs cursor-pointer">
                <Users2Icon size={14} />
                <span>
                  {participantsCount} Participant
                  {participantsCount !== 1 ? "s" : ""}
                </span>
              </PopoverTrigger>
              <PopoverContent
                align="start"
                className="shadow-lg p-4 border-neutral-600 bg-neutral-800"
                closeOnClickOutside={true}
              >
                <div className="space-y-2">
                  <div className="pb-1">
                    <p className="text-xs font-medium text-neutral-500">
                      Participants
                    </p>
                  </div>
                  <div className="space-y-0.5">
                    {uniqueParticipants.map((participant, index) => (
                      <div
                        key={index}
                        className="flex w-full items-start justify-between rounded py-2 text-sm"
                      >
                        <div className="flex w-full items-center">
                          <div className="flex items-center gap-3">
                            <div className="size-6 rounded-full bg-neutral-700 flex items-center justify-center">
                              <span className="text-xs text-neutral-100">
                                {participant.substring(0, 2).toUpperCase()}
                              </span>
                            </div>
                            <span className="font-medium text-neutral-100">
                              {participant}
                            </span>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </PopoverContent>
            </Popover>
          </div>
        )}
      </div>
    </div>
  );
}
