import { mockIPC } from "@tauri-apps/api/mocks";
import { Channel } from "@tauri-apps/api/core";
import { type SessionEvent } from "@hypr/plugin-listener";

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export const mockTranscriptIPC = () =>
  mockIPC((cmd, args) => {
    if (cmd == "plugin:listener|subscribe") {
      const channel: Channel<SessionEvent> = (args as any).channel;

      // Send messages with realistic delays
      (async () => {
        let currentTime = 0;

        // John starts the meeting
        await sleep(500);
        currentTime += 0.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 3,
                speaker: "John",
                text: "Hey team, thanks for joining. Today we'll discuss the new transcription feature requirements.",
              },
            ],
          },
        });

        // Sarah responds
        await sleep(3500);
        currentTime += 3.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 5,
                speaker: "Sarah",
                text: "I've been working on some mockups based on user feedback. The main request is for real-time updates and clear speaker identification.",
              },
            ],
          },
        });

        // Mike adds technical context
        await sleep(5500);
        currentTime += 5.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 4,
                speaker: "Mike",
                text: "That aligns with our backend capabilities. We can stream the transcription with about 500ms latency.",
              },
            ],
          },
        });

        // John asks about timeline
        await sleep(4500);
        currentTime += 4.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 2,
                speaker: "John",
                text: "What's our timeline for implementing this?",
              },
            ],
          },
        });

        // Sarah provides estimate
        await sleep(3000);
        currentTime += 3;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 4,
                speaker: "Sarah",
                text: "The UI work should take about two weeks. We already have most of the components ready.",
              },
            ],
          },
        });

        // Mike confirms backend timeline
        await sleep(4500);
        currentTime += 4.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 3,
                speaker: "Mike",
                text: "Backend integration can be done in parallel. We should be ready for testing in two weeks.",
              },
            ],
          },
        });

        // Meeting wrap-up
        await sleep(3500);
        currentTime += 3.5;
        channel.onmessage({
          type: "timelineView",
          timeline: {
            items: [
              {
                start: currentTime,
                end: currentTime + 2,
                speaker: "John",
                text: "Perfect, let's reconvene next week for a progress check. Thanks everyone!",
              },
            ],
          },
        });

        // Send stopped event after a brief pause
        await sleep(2500);
        channel.onmessage({
          type: "stopped",
        });
      })();
    }

    return {};
  });
