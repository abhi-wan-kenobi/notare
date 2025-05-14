import { NodeViewContent, NodeViewWrapper } from "@tiptap/react";

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@hypr/ui/components/ui/select";
import type { Speaker } from "./nodes";

export const SpeakerView = ({ node, updateAttributes }: any) => {
  const { speakers, speakerId } = node.attrs as { speakers: Speaker[]; speakerId: string };

  const selectedSpeaker = speakers.find((s) => s.id === speakerId);
  const displayName = selectedSpeaker?.name || speakerId;

  const handleChange = (speakerId: string) => {
    if (speakers.map((s) => s.id).includes(speakerId)) {
      updateAttributes({ speakerId });
    }
  };

  return (
    <NodeViewWrapper className="transcript-speaker">
      <div style={{ width: "140px", padding: "8px" }}>
        <Select value={speakerId} onValueChange={handleChange}>
          <SelectTrigger className="transcript-speaker-select" data-speaker-id={speakerId}>
            <SelectValue placeholder="Select speaker">{displayName}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {speakers.map((speaker: any) => (
              <SelectItem key={speaker.id} value={speaker.id}>
                {speaker.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div style={{ padding: "8px" }}>
        <NodeViewContent />
      </div>
    </NodeViewWrapper>
  );
};
