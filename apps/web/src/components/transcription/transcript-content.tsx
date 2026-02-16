export function TranscriptContent({
  transcript,
}: {
  transcript: string | null;
}) {
  if (!transcript) {
    return (
      <div className="py-8 text-center">
        <div className="flex flex-col items-center gap-3">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-stone-600" />
          <p className="text-sm text-neutral-500">Transcribing...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="prose prose-sm max-w-none">
      <p className="text-neutral-700 leading-relaxed whitespace-pre-wrap">
        {transcript}
      </p>
    </div>
  );
}
