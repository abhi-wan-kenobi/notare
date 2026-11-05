import { WrenchIcon } from "lucide-react";
import { useMemo } from "react";

import { useToolState } from "./shared";

import { CitationChip } from "~/chat/citations/chip";
import { parseTimestampToMs } from "~/chat/citations/parser";
import { Disclosure } from "~/chat/components/message/shared";
import { extractMcpOutputText } from "~/chat/mcp/mcp-output-parser";
import { CONTEXT_TEXT_FIELD } from "~/chat/tools/context-text";

function formatToolName(name: string): string {
  return name.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}

function formatOutputText(output: unknown): string | null {
  const mcpText = extractMcpOutputText(output);
  if (mcpText) {
    return mcpText;
  }

  if (typeof output === "string") {
    return output;
  }

  if (output === null || output === undefined) {
    return null;
  }

  try {
    if (
      typeof output === "object" &&
      output !== null &&
      CONTEXT_TEXT_FIELD in output
    ) {
      const { [CONTEXT_TEXT_FIELD]: _contextText, ...rest } = output as Record<
        string,
        unknown
      >;
      return JSON.stringify(rest, null, 2);
    }

    return JSON.stringify(output, null, 2);
  } catch {
    return String(output);
  }
}

export function ToolGeneric({ part }: { part: Record<string, unknown> }) {
  const toolName = String(
    part.toolName ??
      (typeof part.type === "string" ? part.type.replace("tool-", "") : "tool"),
  );
  const { failed } = useToolState(part as { state: string });
  const done = (part.state as string) === "output-available";

  if (done || failed) {
    const outputText = done ? formatOutputText(part.output) : null;

    return (
      <Disclosure
        icon={<WrenchIcon className="h-3 w-3" />}
        title={
          failed
            ? `${formatToolName(toolName)} failed`
            : formatToolName(toolName)
        }
      >
        <div className="flex flex-col gap-2">
          <InputDisplay input={part.input} />
          {failed ? (
            <p className="text-xs text-red-500">
              {String(part.errorText ?? "Unknown error")}
            </p>
          ) : null}
          {outputText ? <ToolOutputText text={outputText} /> : null}
        </div>
      </Disclosure>
    );
  }

  return (
    <Disclosure
      icon={<WrenchIcon className="h-3 w-3" />}
      title={`Running ${formatToolName(toolName)}…`}
      disabled
    >
      {null}
    </Disclosure>
  );
}

function InputDisplay({ input }: { input: unknown }) {
  if (!input || typeof input !== "object") return null;
  const entries = Object.entries(input as Record<string, unknown>);
  if (entries.length === 0) return null;

  return (
    <dl className="text-muted-foreground flex flex-col gap-1 text-xs">
      {entries.map(([key, value]) => (
        <div key={key}>
          <dt className="text-muted-foreground inline font-medium">{key}: </dt>
          <dd className="inline wrap-break-word whitespace-pre-wrap">
            {typeof value === "string" ? value : JSON.stringify(value)}
          </dd>
        </div>
      ))}
    </dl>
  );
}
type ToolOutputSegment =
  | { type: "text"; text: string }
  | { type: "citation"; ms: number; sessionId?: string };

const TOOL_TIMESTAMP_REGEX =
  /(?:\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]|\[(\d{1,2}:\d{2}(?::\d{2})?)\]|(?:"at_ms"|at_ms)\s*[:=]\s*(\d+))/g;

function parseToolOutputSegments(text: string): ToolOutputSegment[] {
  const segments: ToolOutputSegment[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = TOOL_TIMESTAMP_REGEX.exec(text)) !== null) {
    const [raw, srcSessionId, srcMs, bracketTime, atMs] = match;
    const matchIndex = match.index;

    if (matchIndex > lastIndex) {
      segments.push({
        type: "text",
        text: text.slice(lastIndex, matchIndex),
      });
    }

    if (srcSessionId && srcMs) {
      const ms = Number.parseInt(srcMs, 10);
      if (!Number.isNaN(ms)) {
        segments.push({ type: "citation", ms, sessionId: srcSessionId });
      } else {
        segments.push({ type: "text", text: raw });
      }
    } else if (bracketTime) {
      const ms = parseTimestampToMs(bracketTime);
      if (ms !== null) {
        segments.push({ type: "citation", ms });
      } else {
        segments.push({ type: "text", text: raw });
      }
    } else if (atMs) {
      const ms = Number.parseInt(atMs, 10);
      if (!Number.isNaN(ms)) {
        segments.push({ type: "text", text: "at_ms: " });
        segments.push({ type: "citation", ms });
      } else {
        segments.push({ type: "text", text: raw });
      }
    }

    lastIndex = matchIndex + raw.length;
  }

  if (lastIndex < text.length) {
    segments.push({
      type: "text",
      text: text.slice(lastIndex),
    });
  }

  return segments;
}

export function ToolOutputText({ text }: { text: string }) {
  const segments = useMemo(() => parseToolOutputSegments(text), [text]);

  return (
    <div className="text-muted-foreground text-xs whitespace-pre-wrap">
      {segments.map((segment, i) =>
        segment.type === "text" ? (
          <span key={i}>{segment.text}</span>
        ) : (
          <CitationChip
            key={i}
            ms={segment.ms}
            sessionId={segment.sessionId}
            variant="timestamp"
          />
        ),
      )}
    </div>
  );
}
