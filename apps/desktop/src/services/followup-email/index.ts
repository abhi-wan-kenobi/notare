import { generateText } from "ai";
import { useCallback, useState } from "react";

import { deliverFollowupEmail, type DeliveryResult } from "./delivery";
import {
  FOLLOWUP_SYSTEM_PROMPT,
  assembleFollowupPrompt,
  parseFollowupOutput,
  type FollowupDraft,
} from "./draft";
import { extractExternalAttendees } from "./recipients";

import { useScopedLanguageModel } from "~/ai/hooks/useLLMConnection";
import { liveQueryClient } from "~/db";

export * from "./recipients";
export * from "./mailto";
export * from "./draft";
export * from "./delivery";

interface SessionNoteData {
  title: string | null;
  content: string | null;
  event_json: string | null;
}

export function useFollowupEmail(sessionId: string) {
  const model = useScopedLanguageModel("notes");
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const generateAndDeliver =
    useCallback(async (): Promise<DeliveryResult | null> => {
      if (!sessionId) return null;
      setIsGenerating(true);
      setError(null);

      try {
        // 1. Fetch session title, notes, and calendar event data from local DB
        const rows = await liveQueryClient.execute<{
          title: string | null;
          content: string | null;
          event_json: string | null;
        }>(
          `
          SELECT title, content, event_json
          FROM sessions
          WHERE id = ?
          LIMIT 1
        `,
          [sessionId],
        );

        const session = rows[0] as SessionNoteData | undefined;
        const title = session?.title ?? "Meeting";
        const notesContent = session?.content ?? "";

        let attendees: string[] = [];
        if (session?.event_json) {
          try {
            const parsedEvent = JSON.parse(session.event_json);
            if (Array.isArray(parsedEvent.attendees)) {
              attendees = parsedEvent.attendees.map(
                (a: { email?: string }) => a.email ?? "",
              );
            }
          } catch {
            // ignore event_json parse error
          }
        }

        const externalRecipients = extractExternalAttendees(attendees);

        // 2. Generate email draft using the notes model
        let draft: FollowupDraft;
        if (model) {
          const prompt = assembleFollowupPrompt({
            sessionTitle: title,
            notesContent,
            attendees: externalRecipients,
          });

          const response = await generateText({
            model,
            system: FOLLOWUP_SYSTEM_PROMPT,
            prompt,
          });

          draft = parseFollowupOutput(response.text, `Follow-up: ${title}`);
        } else {
          // Fallback when no LLM connection is configured
          draft = {
            subject: `Follow-up: ${title}`,
            body: notesContent || `Follow-up notes for ${title}`,
          };
        }

        // 3. Deliver via Gmail draft or mailto fallback
        const result = await deliverFollowupEmail({
          to: externalRecipients,
          subject: draft.subject,
          body: draft.body,
        });

        return result;
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        throw err;
      } finally {
        setIsGenerating(false);
      }
    }, [sessionId, model]);

  return {
    generateAndDeliver,
    isGenerating,
    error,
  };
}
