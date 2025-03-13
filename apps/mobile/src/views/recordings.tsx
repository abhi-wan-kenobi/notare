import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useFlow, useLoaderData } from "@stackflow/react/future";
import { UploadIcon } from "lucide-react";
import { useMemo, useState } from "react";

import { RecordingItem } from "../components/recordings/recording-item";
import { LocalRecording, localRecordings } from "../mock/recordings";
import { formatDateHeader, getSortedDatesForRecordings, groupRecordingsByDate } from "../utils/date";

import { Button } from "@hypr/ui/components/ui/button";

export function recordingsLoader({}: ActivityLoaderArgs<"RecordingsView">) {
  return {
    recordings: localRecordings,
  };
}

export const RecordingsView: ActivityComponentType<"RecordingsView"> = () => {
  const { recordings } = useLoaderData<typeof recordingsLoader>();
  const [localRecordingsList] = useState<LocalRecording[]>(recordings);
  const [selectedRecordingId, setSelectedRecordingId] = useState<string | null>(null);

  const { replace } = useFlow();

  const groupedRecordings = useMemo(() => {
    return groupRecordingsByDate(localRecordingsList);
  }, [localRecordingsList]);

  const sortedDates = useMemo(() => {
    return getSortedDatesForRecordings(groupedRecordings);
  }, [groupedRecordings]);

  const handleClickRecording = (recordingId: string) => {
    setSelectedRecordingId(prevId => prevId === recordingId ? null : recordingId);
  };

  const handleConfirmSelection = () => {
    const selectedRecording = localRecordingsList.find(
      (recording) => recording.id === selectedRecordingId,
    );

    if (selectedRecording) {
      // Create a unique note ID based on the recording ID
      const noteId = `note-${selectedRecording.id}`;

      // Navigate to the note view with the new note ID by replacing
      replace("NoteView", { id: noteId });
    }

    // TODO: Implementation for confirming the selection would go here
  };

  return (
    <AppScreen
      appBar={{
        title: "Recordings",
      }}
    >
      <div className="relative flex h-full flex-col">
        <div className="flex-1 overflow-y-auto px-4">
          {localRecordingsList.length === 0
            ? (
              <div className="flex flex-col justify-center items-center h-64">
                <p className="text-neutral-500 mb-4">No recordings yet</p>
              </div>
            )
            : (
              <>
                {sortedDates.map((dateKey: string) => {
                  const { date, recordings: dateRecordings } = groupedRecordings[dateKey];

                  return (
                    <section key={dateKey} className="mb-6">
                      <h2 className="font-bold text-neutral-600 mb-3">
                        {formatDateHeader(date)}
                      </h2>

                      <div className="space-y-2">
                        {dateRecordings.map((recording) => (
                          <RecordingItem
                            key={recording.id}
                            recording={recording}
                            onSelect={() => handleClickRecording(recording.id)}
                            isSelected={selectedRecordingId === recording.id}
                          />
                        ))}
                      </div>
                    </section>
                  );
                })}
              </>
            )}
        </div>

        {selectedRecordingId && (
          <div
            className="absolute z-10 bottom-0 left-0 right-0 flex justify-center px-4 pb-4"
            onClick={(e) => e.stopPropagation()}
          >
            <Button
              className="w-full py-3 text-lg font-semibold"
              onClick={handleConfirmSelection}
            >
              <UploadIcon size={20} className="mr-2" />Upload Recording
            </Button>
          </div>
        )}
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    RecordingsView: {};
  }
}
