import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { json2md, parseJsonContent } from "@hypr/editor/markdown";

import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
import { useEnhancedNoteRecords } from "~/session/queries";
import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

type ChapterSegment = { text: string; start_ms: number; end_ms: number };
export type Chapter = { title: string; startMs: number; durationMs: number };

const STOP_WORDS: Record<string, true> = {
  about: true,
  after: true,
  again: true,
  from: true,
  into: true,
  that: true,
  their: true,
  there: true,
  these: true,
  this: true,
  those: true,
  with: true,
  will: true,
  when: true,
  where: true,
  which: true,
};

export function buildChapters(
  markdown: string,
  segments: ChapterSegment[],
): Chapter[] {
  const headings = markdown
    .replace(/```[\s\S]*?```/g, "")
    .split("\n")
    .flatMap((line) => {
      const match = /^##\s+(.+?)\s*#*\s*$/.exec(line);
      return match ? [match[1]] : [];
    });
  if (!headings.length || !segments.length) return [];

  const ordered = [...segments].sort((a, b) => a.start_ms - b.start_ms);
  const starts: number[] = [];
  headings.forEach((heading, index) => {
    const keywords =
      heading
        .toLocaleLowerCase()
        .match(/[\p{L}\p{N}]{3,}/gu)
        ?.filter((word) => !STOP_WORDS[word]) ?? [];
    let bestIndex = -1;
    let bestScore = 0;
    const lastStart = starts.length > 0 ? starts[starts.length - 1] : -1;
    for (
      let segmentIndex = lastStart + 1;
      segmentIndex < ordered.length;
      segmentIndex += 1
    ) {
      const text = ordered[segmentIndex].text.toLocaleLowerCase();
      const score = keywords.reduce(
        (total, word) => total + (text.includes(word) ? 1 : 0),
        0,
      );
      if (score > bestScore) {
        bestIndex = segmentIndex;
        bestScore = score;
      }
    }
    const proportional =
      headings.length === 1
        ? 0
        : Math.floor((index * (ordered.length - 1)) / (headings.length - 1));
    const previousStart = starts.length > 0 ? starts[starts.length - 1] : -1;
    starts.push(
      bestIndex >= 0 ? bestIndex : Math.max(previousStart + 1, proportional),
    );
  });

  return headings.map((title, index) => {
    const segmentIndex = Math.min(starts[index] ?? 0, ordered.length - 1);
    const segment = ordered[segmentIndex];
    const next = ordered[starts[index + 1] ?? ordered.length];
    const startMs = segment.start_ms;
    const lastSegment =
      ordered.length > 0 ? ordered[ordered.length - 1] : undefined;
    const nextStart = next?.start_ms ?? lastSegment?.end_ms ?? startMs;
    return { title, startMs, durationMs: Math.max(0, nextStart - startMs) };
  });
}

function toMarkdown(content: string) {
  if (!content.trim()) return "";
  if (!content.trim().startsWith("{")) return content;
  try {
    return json2md(parseJsonContent(content));
  } catch {
    return content;
  }
}

export function useChapters(
  sessionId: string,
  markdownProp?: string,
): Chapter[] {
  const enhancedNotes = useEnhancedNoteRecords(sessionId);
  const markdown = useMemo(() => {
    if (markdownProp !== undefined) return markdownProp;
    const first = enhancedNotes[0];
    return first ? toMarkdown(first.content) : "";
  }, [markdownProp, enhancedNotes]);

  const { request } = useSessionTranscriptRenderData(sessionId);
  const requestKey = useMemo(
    () => getRenderTranscriptRequestKey(request),
    [request],
  );
  const { data = [] } = useQuery({
    queryKey: ["meeting-chapters", sessionId, requestKey],
    queryFn: () => (request ? renderTranscriptSegments(request) : []),
    enabled: Boolean(request),
    staleTime: Number.POSITIVE_INFINITY,
  });
  return useMemo(() => buildChapters(markdown, data), [data, markdown]);
}
