import {
  type ChatTransport,
  convertToModelMessages,
  type LanguageModel,
  stepCountIs,
  ToolLoopAgent,
  type ToolSet,
} from "ai";

import {
  type SessionContext,
  commands as templateCommands,
} from "@hypr/plugin-template";

import type { ContextRef } from "../context/entities";
import { CONTEXT_TEXT_FIELD } from "../tools";
import type { HyprUIMessage } from "../types";
import {
  getContextRefs,
  getSessionIdsFromSearchOutput,
  hasContextText,
  isRecord,
  isToolOutputPart,
  MAX_TOOL_STEPS,
  MESSAGE_WINDOW_SIZE,
  MESSAGE_WINDOW_THRESHOLD,
  type ToolOutputPart,
} from "./helpers";

export class CustomChatTransport implements ChatTransport<HyprUIMessage> {
  constructor(
    private model: LanguageModel,
    private tools: ToolSet,
    private systemPrompt?: string,
    private resolveContextRef?: (
      ref: ContextRef,
    ) => Promise<SessionContext | null>,
  ) {}

  private async renderContextBlock(
    contextRefs: ContextRef[],
    cache: Map<string, string | null>,
  ): Promise<string | null> {
    if (!this.resolveContextRef || contextRefs.length === 0) {
      return null;
    }

    const cacheKey = JSON.stringify(contextRefs);
    if (cache.has(cacheKey)) {
      return cache.get(cacheKey) ?? null;
    }

    const seen = new Set<string>();
    const contexts: SessionContext[] = [];
    for (const ref of contextRefs) {
      if (seen.has(ref.key)) continue;
      seen.add(ref.key);
      const context = await this.resolveContextRef(ref);
      if (context) contexts.push(context);
    }

    if (contexts.length === 0) {
      cache.set(cacheKey, null);
      return null;
    }

    // Rendered by Rust-side template engine via Tauri plugin
    const rendered = await templateCommands.render({
      contextBlock: { contexts },
    });
    const result = rendered.status === "ok" ? rendered.data : null;
    cache.set(cacheKey, result);
    return result;
  }

  private async hydrateSearchOutput(
    output: unknown,
    cache: Map<string, string | null>,
  ): Promise<unknown> {
    const sessionIds = getSessionIdsFromSearchOutput(output);
    if (sessionIds.length === 0) return output;

    const refs: ContextRef[] = sessionIds.map((sessionId) => ({
      kind: "session" as const,
      key: `session:search:${sessionId}`,
      source: "tool" as const,
      sessionId,
    }));

    const contextText = await this.renderContextBlock(refs, cache);
    if (!contextText) return output;

    return {
      ...(isRecord(output) ? output : {}),
      [CONTEXT_TEXT_FIELD]: contextText,
    };
  }

  private async expandSearchSessionsOutput(
    part: ToolOutputPart,
    cache: Map<string, string | null>,
  ): Promise<ToolOutputPart> {
    if (hasContextText(part.output)) {
      return part;
    }

    const output = await this.hydrateSearchOutput(part.output, cache);
    if (output === part.output) return part;

    return {
      ...part,
      output,
    };
  }

  private buildHydratingToolSet(cache: Map<string, string | null>): ToolSet {
    const searchTool = this.tools.search_sessions;
    if (!searchTool || typeof searchTool !== "object") {
      return this.tools;
    }

    const execute = (
      searchTool as {
        execute?: (...args: unknown[]) => Promise<unknown>;
      }
    ).execute;
    if (typeof execute !== "function") {
      return this.tools;
    }

    return {
      ...this.tools,
      search_sessions: {
        ...searchTool,
        execute: async (...args: unknown[]) => {
          const output = await execute(...args);
          if (hasContextText(output)) {
            return output;
          }
          return this.hydrateSearchOutput(output, cache);
        },
      },
    };
  }

  sendMessages: ChatTransport<HyprUIMessage>["sendMessages"] = async (
    options,
  ) => {
    const cache = new Map<string, string | null>();
    const tools = this.buildHydratingToolSet(cache);

    const agent = new ToolLoopAgent({
      model: this.model,
      instructions: this.systemPrompt,
      tools,
      stopWhen: stepCountIs(MAX_TOOL_STEPS),
      prepareStep: async ({ messages }) => {
        if (messages.length > MESSAGE_WINDOW_THRESHOLD) {
          return { messages: messages.slice(-MESSAGE_WINDOW_SIZE) };
        }
        return {};
      },
    });

    const messagesWithContext: HyprUIMessage[] = [];

    for (const msg of options.messages) {
      if (msg.role === "user") {
        const contextRefs = getContextRefs(msg.metadata);
        if (contextRefs.length === 0) {
          messagesWithContext.push(msg);
          continue;
        }

        const contextBlock = await this.renderContextBlock(contextRefs, cache);
        if (!contextBlock) {
          messagesWithContext.push(msg);
          continue;
        }

        messagesWithContext.push({
          ...msg,
          parts: [
            { type: "text" as const, text: `${contextBlock}\n\n` },
            ...msg.parts,
          ],
        });
      } else if (msg.role === "assistant") {
        const expandedParts = await Promise.all(
          msg.parts.map((part) => {
            if (
              isToolOutputPart(part) &&
              part.type === "tool-search_sessions"
            ) {
              return this.expandSearchSessionsOutput(part, cache);
            }
            return part;
          }),
        );
        messagesWithContext.push({
          ...msg,
          parts: expandedParts as HyprUIMessage["parts"],
        });
      } else {
        messagesWithContext.push(msg);
      }
    }

    const result = await agent.stream({
      messages: await convertToModelMessages(messagesWithContext),
    });

    return result.toUIMessageStream({
      originalMessages: options.messages,
      messageMetadata: ({ part }: { part: { type: string } }) => {
        if (part.type === "start") {
          return { createdAt: Date.now() };
        }
      },
      onError: (error: unknown) => {
        console.error(error);
        if (error instanceof Error) {
          return `${error.name}: ${error.message}`;
        }
        if (isRecord(error) && typeof error.message === "string") {
          return error.message;
        }
        try {
          return JSON.stringify(error);
        } catch {
          return String(error);
        }
      },
    });
  };

  reconnectToStream: ChatTransport<HyprUIMessage>["reconnectToStream"] =
    async () => {
      return null;
    };
}
