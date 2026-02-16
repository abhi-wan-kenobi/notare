import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Streamdown } from "streamdown";

import type { JSONContent } from "@hypr/tiptap/editor";
import NoteEditor from "@hypr/tiptap/editor";
import { EMPTY_TIPTAP_DOC, md2json } from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";
import { cn } from "@hypr/utils";

import { EMPTY_MENTION_CONFIG } from "./constants";

export function SummaryView({
  summary,
  isStreaming,
  error,
  onRegenerate,
}: {
  summary: string;
  isStreaming: boolean;
  error: string | null;
  onRegenerate: () => void;
}) {
  const [editableContent, setEditableContent] =
    useState<JSONContent>(EMPTY_TIPTAP_DOC);
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    if (!isStreaming && summary) {
      try {
        const json = md2json(summary);
        setEditableContent(json);
        setIsEditing(true);
      } catch {
        setIsEditing(false);
      }
    }
  }, [isStreaming, summary]);

  if (error) {
    return (
      <div className="flex flex-col items-center gap-4 py-12 text-center">
        <p className="text-sm text-red-600">{error}</p>
        <button
          onClick={onRegenerate}
          className={cn([
            "flex items-center gap-2 px-4 py-2 text-sm",
            "border border-neutral-200 rounded-lg",
            "hover:bg-neutral-50 transition-colors",
          ])}
        >
          <RefreshCw size={14} />
          Retry
        </button>
      </div>
    );
  }

  if (isStreaming) {
    if (!summary) {
      return (
        <div className="py-8 text-center">
          <p className="text-sm text-neutral-500">Generating summary...</p>
        </div>
      );
    }

    return (
      <div className="pb-2">
        <Streamdown
          components={streamdownComponents}
          className={cn(["flex flex-col"])}
          caret="block"
          isAnimating={true}
        >
          {summary}
        </Streamdown>
      </div>
    );
  }

  if (isEditing && summary) {
    return (
      <NoteEditor
        key={`summary-${summary.length}`}
        initialContent={editableContent}
        handleChange={setEditableContent}
        mentionConfig={EMPTY_MENTION_CONFIG}
      />
    );
  }

  return (
    <div className="py-8 text-center">
      <p className="text-sm text-neutral-400">
        Summary will appear here after transcription
      </p>
    </div>
  );
}

const HEADING_SHARED = "text-gray-700 font-semibold text-sm mb-1 min-h-6";
const HEADING_WITH_MARGIN = "mt-4 first:mt-0";

const streamdownComponents = {
  h1: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h1 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-xl"])}>
      {props.children as React.ReactNode}
    </h1>
  ),
  h2: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h2 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-lg"])}>
      {props.children as React.ReactNode}
    </h2>
  ),
  h3: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h3 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-base"])}>
      {props.children as React.ReactNode}
    </h3>
  ),
  h4: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h4 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-sm"])}>
      {props.children as React.ReactNode}
    </h4>
  ),
  h5: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h5 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-sm"])}>
      {props.children as React.ReactNode}
    </h5>
  ),
  h6: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h6 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-xs"])}>
      {props.children as React.ReactNode}
    </h6>
  ),
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => (
    <ul className="list-disc pl-6 mb-1 block relative">
      {props.children as React.ReactNode}
    </ul>
  ),
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => (
    <ol className="list-decimal pl-6 mb-1 block relative">
      {props.children as React.ReactNode}
    </ol>
  ),
  li: (props: React.HTMLAttributes<HTMLLIElement>) => (
    <li className="mb-1">{props.children as React.ReactNode}</li>
  ),
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => (
    <p className="mb-1">{props.children as React.ReactNode}</p>
  ),
} as const;
