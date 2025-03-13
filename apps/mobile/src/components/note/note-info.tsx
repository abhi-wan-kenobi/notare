import { CalendarIcon, TagsIcon, Users2Icon } from "lucide-react";
import * as React from "react";

import { type Session } from "@hypr/plugin-db";
import { Avatar, AvatarFallback } from "@hypr/ui/components/ui/avatar";
import { Badge } from "@hypr/ui/components/ui/badge";
import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";
import { Button } from "@hypr/ui/components/ui/button";

interface SessionInfoProps {
  session: Session;
}

// TODO: Change the mock participants data to a real data

const mockParticipants = [
  {
    id: "1",
    full_name: "John Doe",
    job_title: "Product Manager",
    organization_id: "Acme Inc",
  },
  {
    id: "2",
    full_name: "Jane Smith",
    job_title: "Software Engineer",
    organization_id: "Acme Inc",
  },
  {
    id: "3",
    full_name: "Alex Johnson",
    job_title: "Designer",
    organization_id: "Design Studio",
  },
];

// Mock event data
const mockEvent = {
  id: "event-123",
  title: "Weekly Team Sync",
  start_time: "2025-03-13T14:00:00",
  end_time: "2025-03-13T15:00:00",
  location: "Conference Room A",
};

// Mock tags data
const mockTags = [
  { id: "tag-1", name: "Important" },
  { id: "tag-2", name: "Follow-up" },
  { id: "tag-3", name: "Decision" },
  { id: "tag-4", name: "Action Item" },
  { id: "tag-5", name: "Question" },
];

export function NoteInfo({ session }: SessionInfoProps) {
  const [participantsSheetOpen, setParticipantsSheetOpen] = React.useState(false);
  const [calendarSheetOpen, setCalendarSheetOpen] = React.useState(false);
  const [tagsSheetOpen, setTagsSheetOpen] = React.useState(false);

  const currentDate = new Date().toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });

  const formatEventTime = (startTime: string, endTime: string) => {
    const start = new Date(startTime);
    const end = new Date(endTime);

    const startFormat = start.toLocaleTimeString("en-US", {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });

    const endFormat = end.toLocaleTimeString("en-US", {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });

    return `${startFormat} - ${endFormat}`;
  };

  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase();
  };

  const groupedParticipants = {
    "Acme Inc": mockParticipants.filter(p => p.organization_id === "Acme Inc"),
    "Design Studio": mockParticipants.filter(p => p.organization_id === "Design Studio"),
  };

  const handleViewInCalendar = () => {
    // TODO: Implement calendar navigation
    setCalendarSheetOpen(false);
  };

  return (
    <div className="px-4 w-full flex flex-col pb-6">
      <h2 className="text-xl font-medium text-neutral-800 mb-2">
        {session.title || "Untitled"}
      </h2>

      <div className="flex flex-row items-center whitespace-nowrap gap-2 overflow-x-auto scrollbar-none">
        <button
          className="-mx-1.5 flex flex-row items-center gap-1 rounded-md px-2 py-1.5 hover:bg-neutral-100 text-xs"
          onClick={() => setCalendarSheetOpen(true)}
        >
          <CalendarIcon size={14} />
          {currentDate}
        </button>

        <button
          className="flex flex-row items-center gap-1 rounded-md px-2 py-1.5 hover:bg-neutral-100 text-xs"
          onClick={() => setParticipantsSheetOpen(true)}
        >
          <Users2Icon size={14} />
          {mockParticipants.length > 2
            ? (
              <span>
                {mockParticipants[0].full_name} +{mockParticipants.length - 1}
              </span>
            )
            : (
              <span>
                {mockParticipants.length} Participant
                {mockParticipants.length !== 1 ? "s" : ""}
              </span>
            )}
        </button>

        <button
          className="flex flex-row items-center gap-1 rounded-md px-2 py-1.5 hover:bg-neutral-100 text-xs"
          onClick={() => setTagsSheetOpen(true)}
        >
          <TagsIcon size={14} />
          {mockTags.length > 0 && (
            <span>
              {mockTags[0].name} +{mockTags.length - 1}
            </span>
          )}
        </button>
      </div>

      {/* Participants BottomSheet */}
      <BottomSheet
        open={participantsSheetOpen}
        onClose={() => setParticipantsSheetOpen(false)}
      >
        <BottomSheetContent className="bg-white">
          <div className="p-4 space-y-4">
            <h3 className="text-lg font-medium mb-2">Participants</h3>

            <div className="space-y-4">
              {Object.entries(groupedParticipants).map(([orgId, members]) => (
                <div key={orgId} className="space-y-2">
                  <div className="pb-1">
                    <p className="text-xs font-medium text-neutral-500">
                      {orgId}
                    </p>
                  </div>
                  <div className="space-y-2">
                    {members.map((member) => (
                      <div
                        key={member.id}
                        className="flex w-full items-start justify-between rounded py-2 text-sm"
                      >
                        <div className="flex w-full items-center">
                          <div className="flex items-center gap-3">
                            <Avatar
                              className="size-8"
                              style={{ backgroundColor: "gray" }}
                            >
                              <AvatarFallback className="text-xs">
                                {getInitials(member.full_name)}
                              </AvatarFallback>
                            </Avatar>

                            <div className="flex flex-col">
                              <span className="font-medium">{member.full_name}</span>
                              {member.job_title && (
                                <span className="text-xs text-neutral-500">
                                  {member.job_title}
                                </span>
                              )}
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </BottomSheetContent>
      </BottomSheet>

      {/* Calendar Event BottomSheet */}
      <BottomSheet
        open={calendarSheetOpen}
        onClose={() => setCalendarSheetOpen(false)}
      >
        <BottomSheetContent className="bg-white">
          <div className="p-4 space-y-4">
            <h3 className="text-lg font-medium mb-2">Event Details</h3>

            <div className="space-y-3">
              <div className="space-y-1">
                <h4 className="font-medium">{mockEvent.title}</h4>
                <p className="text-sm text-neutral-500">
                  {new Date(mockEvent.start_time).toLocaleDateString("en-US", {
                    weekday: "long",
                    month: "long",
                    day: "numeric",
                    year: "numeric",
                  })}
                </p>
                <p className="text-sm text-neutral-500">
                  {formatEventTime(mockEvent.start_time, mockEvent.end_time)}
                </p>
                {mockEvent.location && <p className="text-sm text-neutral-500">{mockEvent.location}</p>}
              </div>

              <Button
                className="w-full mt-4"
                variant="outline"
                onClick={handleViewInCalendar}
              >
                <CalendarIcon className="mr-2 size-4" />
                View in Calendar
              </Button>
            </div>
          </div>
        </BottomSheetContent>
      </BottomSheet>

      {/* Tags BottomSheet */}
      <BottomSheet
        open={tagsSheetOpen}
        onClose={() => setTagsSheetOpen(false)}
      >
        <BottomSheetContent className="bg-white">
          <div className="p-4 space-y-4">
            <h3 className="text-lg font-medium mb-2">Tags</h3>

            <div className="flex flex-wrap gap-2">
              {mockTags.map((tag) => (
                <Badge
                  key={tag.id}
                  className="px-2 py-1"
                  variant="outline"
                >
                  {tag.name}
                </Badge>
              ))}
            </div>
          </div>
        </BottomSheetContent>
      </BottomSheet>
    </div>
  );
}
