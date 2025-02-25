import { type Extension, fetch } from "@hypr/extension-utils";
import { Button } from "@hypr/ui/components/ui/button";
import { WidgetHeader, WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";
import { Maximize2Icon } from "lucide-react";
import type { TranscriptResponse } from "./types";
import { useQuery } from "@tanstack/react-query";

const widget: Extension["twoByTwo"] = ({ onMaximize }) => {
  const transcript = useQuery({
    queryKey: ["transcript"],
    queryFn: async () => {
      const response = await fetch("http://localhost:1234/api/timeline");
      if (!response.ok) {
        throw new Error("Failed to fetch transcript");
      }
      return response.json();
    },
  });

  return (
    <WidgetTwoByTwo>
      <div className="p-4 pb-0">
        <WidgetHeader
          title="Transcript"
          actions={[
            <Button
              variant="ghost"
              size="icon"
              onClick={onMaximize}
              className="p-0"
            >
              <Maximize2Icon size={16} />
            </Button>,
          ]}
        />
      </div>

      <div className="overflow-auto flex-1 p-4">
        <Transcript transcript={transcript.data} />
      </div>
    </WidgetTwoByTwo>
  );
};

const formatTime = (seconds: number) => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${remainingSeconds.toString().padStart(2, "0")}`;
  }
  return `${minutes.toString().padStart(2, "0")}:${remainingSeconds.toString().padStart(2, "0")}`;
};

const Transcript = ({
  transcript,
}: {
  transcript: TranscriptResponse | null | undefined;
}) => {
  if (!transcript) {
    return null;
  }

  return (
    <div className="flex flex-col space-y-4">
      {transcript.timeline.items.map((item, index) => (
        <div
          key={index}
          className="flex flex-col bg-white rounded-lg p-3 shadow-sm border border-neutral-100"
        >
          <div className="flex items-center gap-2 text-xs text-neutral-500 mb-1">
            <span className="font-medium text-neutral-700">{item.speaker}</span>
            <span>•</span>
            <span>
              {formatTime(item.start)}~{formatTime(item.end)}
            </span>
          </div>
          <p className="text-sm text-neutral-800 leading-relaxed">
            {item.text}
          </p>
        </div>
      ))}
    </div>
  );
};

export default widget;
