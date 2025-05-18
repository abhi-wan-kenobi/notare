import { mergeAttributes, Node } from "@tiptap/core";
import { ReactNodeViewRenderer } from "@tiptap/react";

import { SpeakerView } from "./views";

export interface Speaker {
  id: string;
  name: string;
}

export const createSpeakerNode = (speakers: Speaker[]) => {
  const defaultSpeaker = speakers.length > 0 ? speakers[0] : null;

  return Node.create({
    name: "speaker",
    group: "block",
    content: "word*",
    addAttributes() {
      return {
        speakerId: {
          default: defaultSpeaker?.id,
          parseHTML: element => {
            const speakerId = element.getAttribute("data-speaker-id");
            return speakerId && speakers.some(s => s.id === speakerId) ? speakerId : defaultSpeaker?.id;
          },
          renderHTML: attributes => {
            const speakerId = speakers.some(s => s.id === attributes.speakerId)
              ? attributes.speakerId
              : defaultSpeaker?.id;
            return { "data-speaker-id": speakerId };
          },
        },
        speakers: {
          default: speakers,
          renderHTML: () => ({}),
        },
      };
    },
    parseHTML() {
      return [{ tag: "div.transcript-speaker" }];
    },
    renderHTML({ HTMLAttributes, node }) {
      const speakerIds = (node.attrs.speakers || []).map((s: Speaker) => s.id);
      const speakerId = node.attrs.speakerId && speakerIds.includes(node.attrs.speakerId)
        ? node.attrs.speakerId
        : defaultSpeaker?.id;

      return [
        "div",
        mergeAttributes({ class: "transcript-speaker" }, HTMLAttributes),
        ["div", { class: "transcript-speaker-label" }, [
          "select",
          {
            class: "transcript-speaker-select",
            "data-speaker-id": speakerId,
          },
          ...speakers.map(
            speaker => ["option", { value: speaker.id, selected: speakerId === speaker.id }, speaker.name],
          ),
        ]],
        ["div", { class: "transcript-speaker-content" }, 0],
      ];
    },
    addNodeView() {
      return ReactNodeViewRenderer(SpeakerView);
    },
  });
};

export const WordNode = Node.create({
  name: "word",
  group: "inline",
  inline: true,
  atom: false,
  content: "text*",
  addAttributes() {
    return {
      start_ms: {
        default: null,
        parseHTML: element => {
          const value = element.getAttribute("data-start-ms");
          return value !== null ? Number(value) : null;
        },
        renderHTML: attributes => attributes.start_ms != null ? { "data-start-ms": attributes.start_ms } : {},
      },
      end_ms: {
        default: null,
        parseHTML: element => {
          const value = element.getAttribute("data-end-ms");
          return value !== null ? Number(value) : null;
        },
        renderHTML: attributes => attributes.end_ms != null ? { "data-end-ms": attributes.end_ms } : {},
      },
      confidence: {
        default: null,
        parseHTML: element => {
          const value = element.getAttribute("data-confidence");
          return value !== null ? Number(value) : null;
        },
        renderHTML: attributes => attributes.confidence != null ? { "data-confidence": attributes.confidence } : {},
      },
    };
  },
  parseHTML() {
    return [{ tag: "span.transcript-word" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes({ class: "transcript-word" }, HTMLAttributes), 0];
  },
});
