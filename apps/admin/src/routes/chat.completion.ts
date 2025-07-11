import { createServerFileRoute } from "@tanstack/react-start/server";

import { createOpenAI } from "@ai-sdk/openai";
import { generateText, streamText } from "ai";

export const ServerRoute = createServerFileRoute("/chat/completion").methods({
  POST: async ({ request }) => {
    const openai = createOpenAI({
      baseURL: "TODO",
      apiKey: "TODO",
      compatibility: "compatible",
    });

    const { model, messages, stream = false } = await request.json();

    if (!stream) {
      const result = await generateText({
        model: openai(model),
        messages,
      });

      return new Response(JSON.stringify(result.response.body as any));
    }

    return streamText({
      model: openai(model),
      messages,
    }).toDataStreamResponse();
  },
});
