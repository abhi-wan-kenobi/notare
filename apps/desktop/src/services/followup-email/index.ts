import { generateText } from "ai";
import { useCallback, useState } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { deliverFollowupEmail, type DeliveryResult } from "./delivery";
import {
  assembleFollowupPrompt,
  FOLLOWUP_SYSTEM_PROMPT,
  type FollowupDraft,
  parseFollowupOutput,
} from "./draft";
import {
  extractExternalAttendees,
  getEmailDomain,
  PUBLIC_EMAIL_DOMAINS,
} from "./recipients";

import { useScopedLanguageModel } from "~/ai/hooks/useLLMConnection";
import { useAuth } from "~/auth";
import { loadNoteFile } from "~/chat/tools/note-files";
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
  const auth = useAuth();
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

        // Fetch transcript via the same loader chat tools use
        let transcriptText: string | null = null;
        try {
          const noteFile = await loadNoteFile(sessionId);
          const transcriptSection = noteFile?.sections.find(
            (s) => s.title === "Transcript",
          );
          transcriptText = transcriptSection?.text ?? null;
        } catch (e) {
          console.warn(
            "[followup-email] failed to load note file transcript",
            e,
          );
        }

        let attendees: string[] = [];
        let currentUserEmail = auth?.session?.user?.email ?? null;

        if (session?.event_json) {
          try {
            const parsedEvent = JSON.parse(session.event_json);
            if (Array.isArray(parsedEvent.attendees)) {
              attendees = parsedEvent.attendees.map(
                (a: { email?: string }) => a.email ?? "",
              );
              if (!currentUserEmail) {
                const self = parsedEvent.attendees.find(
                  (a: {
                    isCurrentUser?: boolean;
                    is_current_user?: boolean;
                    email?: string;
                  }) =>
                    Boolean(a.isCurrentUser || a.is_current_user) &&
                    Boolean(a.email),
                );
                if (self?.email) {
                  currentUserEmail = self.email;
                }
              }
            }
          } catch {
            // ignore event_json parse error
          }
        }

        const userDomain = currentUserEmail
          ? getEmailDomain(currentUserEmail)
          : null;
        const internalDomain =
          userDomain && !PUBLIC_EMAIL_DOMAINS[userDomain] ? userDomain : null;

        const externalRecipients = extractExternalAttendees(attendees, {
          currentUserEmail,
          internalDomain,
        });

        if (externalRecipients.length === 0) {
          const msg = "No external recipients found for this meeting.";
          setError(msg);
          sonnerToast.error(msg);
          return null;
        }

        // 2. Generate email draft using the notes model
        let draft: FollowupDraft;
        if (model) {
          const prompt = assembleFollowupPrompt({
            sessionTitle: title,
            notesContent,
            transcriptText,
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

        if (result.delivered === "mailto" && result.reason) {
          sonnerToast.error(
            `Gmail draft creation failed (${result.reason}). Opened in mail client instead.`,
          );
        }

        return result;
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        sonnerToast.error(msg);
        throw err;
      } finally {
        setIsGenerating(false);
      }
    }, [sessionId, model, auth?.session?.user?.email]);

  return {
    generateAndDeliver,
    isGenerating,
    error,
  };
}
