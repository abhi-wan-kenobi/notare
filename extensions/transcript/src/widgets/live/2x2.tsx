import { formatTime } from "@hypr/extension-utils";
import { Button } from "@hypr/ui/components/ui/button";
import { WidgetHeader, WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";
import { Badge } from "@hypr/ui/components/ui/badge";
import { useEffect, useState, useRef } from "react";
import { Channel } from "@tauri-apps/api/core";
import {
  commands as listenerCommands,
  type TimelineView,
  type SessionEvent,
} from "@hypr/plugin-listener";
import { useQuery } from "@tanstack/react-query";
import { fetch } from "@hypr/extension-utils";
import { Maximize2Icon } from "lucide-react";

const LiveTranscript2x2: typeof WidgetTwoByTwo = ({ onMaximize }) => {
  const [timeline, setTimeline] = useState<TimelineView | null>(null);
  const [isLive, setIsLive] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const channel = new Channel<SessionEvent>();
    listenerCommands.subscribe(channel);

    channel.onmessage = (e) => {
      if (e.type === "timelineView") {
        setIsLive(true);
        setTimeline((prev) => {
          if (!prev) return e.timeline;
          return {
            items: [...prev.items, ...e.timeline.items],
          };
        });
      }

      if (e.type === "stopped") {
        setIsLive(false);
      }
    };
  }, []);

  // Auto-scroll when new items are added
  useEffect(() => {
    if (scrollRef.current && isLive) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [timeline?.items.length, isLive]);

  const transcript = useQuery({
    queryKey: ["transcript"],
    queryFn: async () => {
      const response = await fetch("/api/timeline");
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
          title={
            <div className="flex items-center gap-2">
              Transcript
              {isLive && <Badge variant="destructive">LIVE</Badge>}
            </div>
          }
          actions={[
            <Button
              key="maximize"
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

      <div ref={scrollRef} className="overflow-y-auto flex-1 p-4 pt-0">
        <Transcript transcript={timeline || transcript.data} />
      </div>
    </WidgetTwoByTwo>
  );
};

const Transcript = ({ transcript }: { transcript: TimelineView | null }) => {
  if (!transcript) {
    return null;
  }

  return (
    <div className="flex flex-col space-y-4">
      {transcript.items.map((item, index) => (
        <div
          key={index}
          className="flex flex-col bg-white rounded-lg p-3 shadow-sm border border-neutral-100"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">{item.speaker}</span>
            <span className="text-xs text-neutral-500">
              {formatTime(item.start)}~{formatTime(item.end)}
            </span>
          </div>
          <p className="text-sm mt-1">{item.text}</p>
        </div>
      ))}
    </div>
  );
};

export default LiveTranscript2x2;
