import { useRef } from "react";

import { cn } from "@hypr/utils";

import type { PipelineStatus } from "@/hooks/use-transcription-pipeline";

const pillClasses = cn([
  "flex items-center gap-2 px-5 py-2.5",
  "rounded-full border-2 border-neutral-200 bg-white",
  "shadow-lg",
]);

export function FloatingCTA({
  status,
  progress,
  onFileSelect,
}: {
  status: PipelineStatus;
  progress: number;
  onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-40">
      <input
        ref={fileInputRef}
        type="file"
        accept="audio/*"
        onChange={onFileSelect}
        className="hidden"
      />

      {status === "uploading" ? (
        <div className={pillClasses}>
          <div className="animate-spin rounded-full h-4 w-4 border-2 border-stone-300 border-t-stone-600" />
          <span className="text-sm font-medium text-neutral-700">
            Uploading... {progress}%
          </span>
        </div>
      ) : status === "uploaded" || status === "transcribing" ? (
        <div className={pillClasses}>
          <div className="animate-spin rounded-full h-4 w-4 border-2 border-stone-300 border-t-stone-600" />
          <span className="text-sm font-medium text-neutral-700">
            Transcribing...
          </span>
        </div>
      ) : (
        <button
          onClick={() => fileInputRef.current?.click()}
          className={cn([
            "flex items-center gap-2 px-5 py-2.5",
            "rounded-full border-2 border-neutral-200 bg-white",
            "shadow-lg",
            "hover:border-neutral-300 hover:shadow-xl",
            "active:scale-[98%]",
            "transition-all",
          ])}
        >
          <span className="flex h-2.5 w-2.5 rounded-full bg-red-400" />
          <span className="text-sm font-medium text-neutral-700">
            Upload file
          </span>
        </button>
      )}
    </div>
  );
}
