import { format } from "date-fns";
import { CheckIcon } from "lucide-react";
import { type LocalRecording } from "../../mock/recordings";
import { formatFileSize, formatRecordingDuration } from "../../utils";

export const RecordingItem = ({
  recording,
  onSelect,
  isSelected = false,
}: {
  recording: LocalRecording;
  onSelect: () => void;
  isSelected?: boolean;
}) => {
  const recordingDate = new Date(recording.created_at);

  return (
    <div
      className={`hover:bg-neutral-100 group flex items-start justify-between gap-3 py-3 w-full text-left transition-all rounded-lg px-3 border ${
        isSelected ? "border-neutral-700 bg-neutral-50" : "border-neutral-200"
      }`}
    >
      <button
        onClick={onSelect}
        className="flex-1 text-left flex items-center justify-between gap-2"
      >
        <div className="flex flex-col items-start gap-1">
          <div className="font-medium text-sm line-clamp-1">
            {recording.title || recording.filename}
          </div>
          <div className="flex items-center gap-2 text-xs text-neutral-500">
            <span>{format(recordingDate, "M/d/yy")}</span>
            <span>•</span>
            <span>{formatRecordingDuration(recording.duration)}</span>
            <span>•</span>
            <span>{formatFileSize(recording.size)}</span>
          </div>
        </div>

        {isSelected && <CheckIcon className="size-4 text-neutral-700 flex-shrink-0" />}
      </button>
    </div>
  );
};
