import { generateText, type LanguageModel } from "ai";

import { assembleBriefPrompt, retrieveBriefContext } from "./prompt";
import { saveBrief } from "./storage";
import type { BriefCandidateEvent, SessionBrief } from "./types";

import { deterministicGenerationSettings } from "~/ai/model-settings";
import { liveQueryClient } from "~/db";

function withResolvers<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

class RateLimiter {
  private queue: Array<() => void> = [];
  private active = false;

  async acquire(): Promise<() => void> {
    if (!this.active) {
      this.active = true;
      return this.makeRelease();
    }

    const { promise, resolve } = withResolvers<void>();
    this.queue.push(resolve);
    await promise;
    return this.makeRelease();
  }
  private makeRelease(): () => void {
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const next = this.queue.shift();
      if (next) {
        next();
      } else {
        this.active = false;
      }
    };
  }

  isBusy(): boolean {
    return this.active;
  }
}

export const briefGenerationRateLimiter = new RateLimiter();

export async function generateBriefForEvent(
  event: BriefCandidateEvent,
  model: LanguageModel,
  modelName: string,
  options: { signal?: AbortSignal } = {},
): Promise<SessionBrief | null> {
  const release = await briefGenerationRateLimiter.acquire();
  try {
    if (options.signal?.aborted) {
      return null;
    }

    const context = await retrieveBriefContext(event);
    const { system, prompt } = assembleBriefPrompt(event, context);

    const result = await generateText({
      model,
      ...deterministicGenerationSettings(model),
      system,
      prompt,
      abortSignal: options.signal,
    });

    const contentMd = result.text.trim();
    if (!contentMd) {
      return null;
    }

    return await saveBrief({
      event_id: event.id,
      content_md: contentMd,
      model: modelName,
    });
  } catch (error) {
    if (options.signal?.aborted) {
      return null;
    }
    console.error("[briefs] failed to generate brief", error);
    return null;
  } finally {
    release();
  }
}

export async function regenerateBriefForEvent(
  eventId: string,
  model: LanguageModel,
  modelName: string,
): Promise<SessionBrief | null> {
  const [event] = await liveQueryClient.execute<BriefCandidateEvent>(
    `
      SELECT
        id,
        title,
        started_at,
        ended_at,
        location,
        meeting_link,
        description,
        calendar_id,
        recurrence_series_id,
        participants_json,
        is_all_day
      FROM events
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [eventId],
  );

  if (!event) {
    return null;
  }

  return generateBriefForEvent(event, model, modelName);
}
