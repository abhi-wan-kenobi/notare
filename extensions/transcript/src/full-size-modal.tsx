import { type Extension, fetch } from "@hypr/extension-utils";
import { Button } from "@hypr/ui/components/ui/button";
import {
  WidgetFullSizeModal,
  WidgetHeader,
} from "@hypr/ui/components/ui/widgets";
import { Minimize2Icon } from "lucide-react";
import type { TranscriptResponse } from "./types";
import { useQuery } from "@tanstack/react-query";
import { Badge } from "@hypr/ui/components/ui/badge";

const widget: Extension["full"] = ({ onMinimize }) => {
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
    <WidgetFullSizeModal onMinimize={onMinimize}>
      <div className="p-4 pb-0">
        <WidgetHeader
          title={
            <div className="flex items-center gap-2">
              Transcript<Badge>LIVE</Badge>
            </div>
          }
          actions={[
            <Button
              variant="ghost"
              size="icon"
              onClick={onMinimize}
              className="p-0"
            >
              <Minimize2Icon size={16} />
            </Button>,
          ]}
        />
      </div>

      <div className="overflow-auto flex-1 p-4">
        <Transcript transcript={transcript.data} />
      </div>
    </WidgetFullSizeModal>
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
