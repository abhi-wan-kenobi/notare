/**
 * Download-on-first-run for the semantic-search model (WS-B2 RC gate).
 *
 * The dense arm is inert until the EmbeddingGemma artifacts are on disk. This
 * hook exposes the model's presence + a `download()` that streams the pinned,
 * SHA-256-verified artifacts (via the `download_embedding_model` plugin command)
 * with progress. Wire `ensureModel()` into the first semantic search / the
 * search page's setup state.
 */

import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands as embedding, type DownloadProgress } from "@hypr/plugin-embedding-search";

export type ModelState = "checking" | "absent" | "downloading" | "ready" | "error";

export type EmbeddingModel = {
  state: ModelState;
  /** 0..1 across all artifacts, while downloading. */
  progress: number;
  error: string | null;
  /** Start (or no-op if already present) the download. Resolves when ready. */
  ensureModel: () => Promise<boolean>;
  refresh: () => Promise<void>;
};

export function useEmbeddingModel(): EmbeddingModel {
  const [state, setState] = useState<ModelState>("checking");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const inFlight = useRef<Promise<boolean> | null>(null);

  const refresh = useCallback(async () => {
    const res = await embedding.embeddingIndexStatus();
    if (res.status === "ok") {
      setState(res.data.modelDownloaded ? "ready" : "absent");
    } else {
      setState("error");
      setError(res.error);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const ensureModel = useCallback(async (): Promise<boolean> => {
    if (state === "ready") return true;
    if (inFlight.current) return inFlight.current;

    const run = (async () => {
      setState("downloading");
      setProgress(0);
      setError(null);
      const channel = new Channel<DownloadProgress>();
      channel.onmessage = (p) => {
        if (p.total > 0) setProgress(Math.min(1, p.downloaded / p.total));
      };
      const res = await embedding.downloadEmbeddingModel(channel);
      if (res.status === "ok") {
        setState("ready");
        setProgress(1);
        return true;
      }
      setState("error");
      setError(res.error);
      return false;
    })();

    inFlight.current = run;
    try {
      return await run;
    } finally {
      inFlight.current = null;
    }
  }, [state]);

  return { state, progress, error, ensureModel, refresh };
}
