import { createFileRoute } from "@tanstack/react-router";
import { RefreshCw } from "lucide-react";
import { lazy, Suspense, useState } from "react";

import type { JSONContent } from "@hypr/tiptap/editor";
import { EMPTY_TIPTAP_DOC } from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";
import { cn } from "@hypr/utils";

import { EMPTY_MENTION_CONFIG } from "@/components/transcription/constants";
import { FloatingCTA } from "@/components/transcription/floating-cta";
import { SummaryView } from "@/components/transcription/summary-view";
import { TabButton } from "@/components/transcription/tab-button";
import { TranscriptContent } from "@/components/transcription/transcript-content";
import { useTranscriptionPipeline } from "@/hooks/use-transcription-pipeline";

const NoteEditor = lazy(() => import("@hypr/tiptap/editor"));

type Tab = "summary" | "memos" | "transcript";

export const Route = createFileRoute("/_view/app/file-transcription")({
  component: Component,
  validateSearch: (search: Record<string, unknown>) => ({
    id: (search.id as string) || undefined,
  }),
});

function Component() {
  const { id: searchId } = Route.useSearch();

  const [noteContent, setNoteContent] = useState<JSONContent>(EMPTY_TIPTAP_DOC);
  const [activeTab, setActiveTab] = useState<Tab>("summary");

  const {
    file,
    transcript,
    summary,
    isSummarizing,
    summaryError,
    uppyProgress,
    status,
    errorMessage,
    handleFileSelect,
    handleRegenerate,
  } = useTranscriptionPipeline(searchId);

  const hasTabs = status === "done" || status === "transcribing";
  const title = file ? file.name.replace(/\.[^.]+$/, "") : "";

  return (
    <div className="relative min-h-[calc(100vh-200px)] pb-24">
      <div className="max-w-3xl mx-auto px-6 pt-10">
        <h1
          className={cn([
            "text-xl font-semibold border-none bg-transparent focus:outline-hidden w-full",
            !title && "text-muted-foreground",
          ])}
        >
          {title || "Untitled"}
        </h1>

        {errorMessage && (
          <div className="mt-3 border border-red-200 bg-red-50 rounded-xs px-4 py-2">
            <p className="text-sm text-red-600">{errorMessage}</p>
          </div>
        )}

        {hasTabs ? (
          <div className="mt-4 flex flex-col">
            <div className="flex items-center gap-1">
              <TabButton
                label="Summary"
                active={activeTab === "summary"}
                onClick={() => setActiveTab("summary")}
                trailing={
                  activeTab === "summary" && transcript && !isSummarizing ? (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRegenerate();
                      }}
                      className="ml-1 p-0.5 rounded hover:bg-neutral-200 transition-colors"
                    >
                      <RefreshCw size={12} />
                    </button>
                  ) : isSummarizing ? (
                    <div className="ml-1 animate-spin rounded-full h-3 w-3 border-b border-stone-600" />
                  ) : null
                }
              />
              <TabButton
                label="Memos"
                active={activeTab === "memos"}
                onClick={() => setActiveTab("memos")}
              />
              <TabButton
                label="Transcript"
                active={activeTab === "transcript"}
                onClick={() => setActiveTab("transcript")}
              />
            </div>

            <div className="mt-2 min-h-[300px]">
              {activeTab === "summary" && (
                <SummaryView
                  summary={summary}
                  isStreaming={isSummarizing}
                  error={summaryError}
                  onRegenerate={handleRegenerate}
                />
              )}

              {activeTab === "memos" && (
                <Suspense fallback={null}>
                  <NoteEditor
                    initialContent={noteContent}
                    handleChange={setNoteContent}
                    mentionConfig={EMPTY_MENTION_CONFIG}
                  />
                </Suspense>
              )}

              {activeTab === "transcript" && (
                <TranscriptContent transcript={transcript} />
              )}
            </div>
          </div>
        ) : (
          <div className="mt-4">
            <Suspense fallback={null}>
              <NoteEditor
                initialContent={noteContent}
                handleChange={setNoteContent}
                mentionConfig={EMPTY_MENTION_CONFIG}
              />
            </Suspense>
          </div>
        )}
      </div>

      {!hasTabs && (
        <FloatingCTA
          status={status}
          progress={uppyProgress}
          onFileSelect={handleFileSelect}
        />
      )}
    </div>
  );
}
