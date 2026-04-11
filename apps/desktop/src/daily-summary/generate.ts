import { generateText, Output } from "ai";
import { z } from "zod";

import { commands as templateCommands } from "@hypr/plugin-template";

import { type DailySummarySnapshot } from "./api";
import { formatSignalTime, formatTime, toError } from "./helpers";
import { dailySummarySchema } from "./types";

import { type useLanguageModel } from "~/ai/hooks";

export async function getDailySummarySystemPrompt() {
  const result = await templateCommands.render({
    dailySummarySystem: {
      language: null,
    },
  });

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

export async function getDailySummaryUserPrompt(params: {
  date: string;
  tz?: string;
  snapshot: DailySummarySnapshot;
}) {
  const { date, tz, snapshot } = params;
  const analyses = snapshot.analyses.slice(-120).map((analysis) => ({
    time: formatTime(analysis.capturedAtMs, tz),
    appName: analysis.appName,
    windowTitle: analysis.windowTitle,
    reason: analysis.reason,
    summary: analysis.summary,
  }));
  const result = await templateCommands.render({
    dailySummaryUser: {
      date,
      timezone: tz ?? null,
      stats: {
        signalCount: snapshot.stats.signalCount,
        screenshotCount: snapshot.stats.screenshotCount,
        analysisCount: snapshot.stats.analysisCount,
        uniqueAppCount: snapshot.stats.uniqueAppCount,
        firstSignal: formatSignalTime(snapshot.stats.firstSignalAtMs, tz),
        lastSignal: formatSignalTime(snapshot.stats.lastSignalAtMs, tz),
      },
      topApps: snapshot.stats.topApps.map((item) => ({
        appName: item.appName,
        count: item.count,
      })),
      analyses,
      totalAnalysisCount: snapshot.analyses.length,
      existingSummary: snapshot.summary?.content?.trim() || null,
    },
  });

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

function createJsonOutputPrompt<T extends z.ZodTypeAny>(
  prompt: string,
  schema: T,
) {
  return `${prompt}

JSON schema:
${JSON.stringify(z.toJSONSchema(schema))}

Return only a JSON object that matches the schema exactly. Start with { and do not wrap the response in markdown fences.`;
}

export async function generateDailySummaryOutput(params: {
  model: ReturnType<typeof useLanguageModel>;
  system: string;
  prompt: string;
}) {
  const { model, system, prompt } = params;

  try {
    const result = await generateText({
      model: model!,
      temperature: 0,
      output: Output.object({ schema: dailySummarySchema }),
      system,
      prompt,
    });

    if (!result.output) {
      throw new Error("Model returned no summary.");
    }

    return result.output;
  } catch (error) {
    console.error("Daily summary structured generation failed:", error);

    const fallbackResult = await generateText({
      model: model!,
      temperature: 0,
      system,
      prompt: createJsonOutputPrompt(prompt, dailySummarySchema),
    });
    const jsonMatch = fallbackResult.text.match(/\{[\s\S]*\}/);

    if (!jsonMatch) {
      throw toError(error);
    }

    try {
      return dailySummarySchema.parse(JSON.parse(jsonMatch[0]));
    } catch (parseError) {
      console.error("Daily summary fallback parsing failed:", parseError, {
        text: fallbackResult.text,
      });
      throw toError(error);
    }
  }
}
