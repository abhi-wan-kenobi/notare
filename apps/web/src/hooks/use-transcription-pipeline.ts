import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { sttListenBatch, sttStatus } from "@hypr/api-client";
import type {
  ListenCallbackResponse,
  SttStatusResponse,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAudioUppy } from "@/hooks/use-audio-uppy";
import { useSummaryStream } from "@/hooks/use-summary-stream";

export type PipelineStatus =
  | "idle"
  | "uploading"
  | "uploaded"
  | "transcribing"
  | "done"
  | "error";

function createAuthClient(accessToken: string) {
  return createClient({
    baseUrl: env.VITE_API_URL,
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

function extractTranscript(response: SttStatusResponse): string | null {
  if (response.status !== "done" || !response.rawResult) return null;

  if (typeof response.rawResult.text === "string") {
    return response.rawResult.text;
  }

  const results = response.rawResult.results as
    | { channels?: Array<{ alternatives?: Array<{ transcript?: string }> }> }
    | undefined;
  const transcript = results?.channels?.[0]?.alternatives?.[0]?.transcript;
  if (typeof transcript === "string") {
    return transcript;
  }

  return null;
}

export function useTranscriptionPipeline(searchId: string | undefined) {
  const [file, setFile] = useState<File | null>(null);
  const [pipelineId, setPipelineId] = useState<string | null>(searchId ?? null);
  const [transcript, setTranscript] = useState<string | null>(null);

  const summaryTriggeredRef = useRef(false);

  const {
    summary,
    isStreaming: isSummarizing,
    error: summaryError,
    generate: generateSummary,
  } = useSummaryStream();

  const {
    addFile: uppyAddFile,
    status: uppyStatus,
    progress: uppyProgress,
    fileId: uppyFileId,
    error: uppyError,
  } = useAudioUppy();

  const startPipelineMutation = useMutation({
    mutationFn: async (fileId: string) => {
      const token = await getAccessToken();
      const client = createAuthClient(token);
      const { data, error } = await sttListenBatch({
        client,
        body: { url: fileId },
        query: { callback: "true", provider: "deepgram" },
      });
      if (error || !data) {
        throw new Error("Failed to start transcription");
      }
      return (data as unknown as ListenCallbackResponse).request_id;
    },
    onSuccess: (newPipelineId) => {
      setPipelineId(newPipelineId);
      const url = new URL(window.location.href);
      url.searchParams.set("id", newPipelineId);
      window.history.replaceState({}, "", url.toString());
    },
  });

  const pipelineStatusQuery = useQuery({
    queryKey: ["audioPipelineStatus", pipelineId],
    queryFn: async (): Promise<SttStatusResponse> => {
      if (!pipelineId) {
        throw new Error("Missing pipelineId");
      }
      const token = await getAccessToken();
      const client = createAuthClient(token);
      const { data, error } = await sttStatus({
        client,
        path: { pipeline_id: pipelineId },
      });
      if (error) {
        throw new Error("Failed to get status");
      }
      return data!;
    },
    enabled: !!pipelineId,
    refetchInterval: (query) => {
      const s = query.state.data?.status;
      return s === "done" || s === "error" ? false : 2000;
    },
  });

  useEffect(() => {
    const data = pipelineStatusQuery.data;
    if (data) {
      const text = extractTranscript(data);
      if (text) {
        setTranscript(text);
      }
    }
  }, [pipelineStatusQuery.data]);

  useEffect(() => {
    if (transcript && !summaryTriggeredRef.current) {
      summaryTriggeredRef.current = true;
      generateSummary(transcript);
    }
  }, [transcript, generateSummary]);

  useEffect(() => {
    if (uppyStatus === "done" && uppyFileId && !pipelineId) {
      startPipelineMutation.mutate(uppyFileId);
    }
  }, [uppyStatus, uppyFileId, pipelineId]);

  const pipelineStatus = pipelineStatusQuery.data?.status;

  const status: PipelineStatus = (() => {
    if (pipelineStatus === "error") return "error";
    if (pipelineStatus === "done" || transcript) return "done";
    if (
      pipelineStatus === "processing" ||
      pipelineId ||
      startPipelineMutation.isPending
    ) {
      return "transcribing";
    }
    if (uppyStatus === "uploading") return "uploading";
    if (uppyStatus === "error") return "error";
    if (uppyStatus === "done" && uppyFileId) return "uploaded";
    return "idle";
  })();

  const errorMessage =
    uppyError ??
    (startPipelineMutation.error instanceof Error
      ? startPipelineMutation.error.message
      : null) ??
    (pipelineStatusQuery.isError && pipelineStatusQuery.error instanceof Error
      ? pipelineStatusQuery.error.message
      : null) ??
    (pipelineStatus === "error"
      ? (pipelineStatusQuery.data?.error ?? null)
      : null);

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = e.target.files?.[0];
    if (!selectedFile) return;
    setFile(selectedFile);
    setPipelineId(null);
    setTranscript(null);
    summaryTriggeredRef.current = false;
    startPipelineMutation.reset();
    uppyAddFile(selectedFile);
  };

  const handleRegenerate = () => {
    if (transcript) {
      summaryTriggeredRef.current = true;
      generateSummary(transcript);
    }
  };

  return {
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
  };
}
