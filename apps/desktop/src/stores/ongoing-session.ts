import { Channel } from "@tauri-apps/api/core";
import { create as mutate } from "mutative";
import { createStore } from "zustand";

import { commands as listenerCommands, type SessionEvent, type TimelineView } from "@hypr/plugin-listener";

type State = {
  sessionId: string | null;
  timeline: TimelineView | null;
  channel: Channel<SessionEvent> | null;
  status: "active" | "loading" | "inactive";
  amplitude: { mic: number; speaker: number };
};

type Actions = {
  start: (sessionId: string) => void;
  pause: () => void;
};

export const createOngoingSessionStore = () => {
  return createStore<State & Actions>((set, get) => ({
    sessionId: null,
    timeline: null,
    status: "inactive",
    channel: null,
    amplitude: { mic: 0, speaker: 0 },
    start: (sessionId: string) => {
      set((state) =>
        mutate(state, (draft) => {
          draft.sessionId = sessionId;
          draft.status = "loading";
        })
      );

      const channel = new Channel<SessionEvent>();

      channel.onmessage = (event) => {
        set((state) =>
          mutate(state, (draft) => {
            if (event.type === "started") {
              draft.status = "active";
            }

            if (event.type === "stopped") {
              draft.status = "inactive";
            }

            if (event.type === "timelineView") {
              console.log("timelineView", event.timeline);
              draft.timeline = event.timeline;
            }

            if (event.type === "audioAmplitude") {
              draft.amplitude = {
                mic: event.mic,
                speaker: event.speaker,
              };
            }
          })
        );
      };

      try {
        listenerCommands.startSession(sessionId).then(() => {
          listenerCommands.subscribe(channel);
          set({ channel, status: "active" });
        });
      } catch (error) {
        console.error("failed to start session", error);
        set({ channel, status: "inactive" });
      }
    },
    pause: () => {
      const { channel } = get();

      try {
        listenerCommands.stopSession();

        if (channel) {
          listenerCommands.unsubscribe(channel);
        }
      } catch (error) {
        console.error(error);
      }
      set({ channel: null, status: "inactive" });
    },
  }));
};
