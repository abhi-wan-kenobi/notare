import { Trans, useLingui } from "@lingui/react/macro";
import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { fetchSttServerStatus, type SttServerStatus } from "./connection-test";

type TestState =
  | { kind: "idle" }
  | { kind: "pending" }
  | { kind: "success"; status: SttServerStatus }
  | { kind: "error"; message: string };

// Technical status details (engine/gpuOffload/model id) are surfaced as
// plain text rather than a Lingui message — same convention as the STT
// connection-health messages in ./health.tsx, which are diagnostic strings,
// not product copy.
function formatSuccessDetail(status: SttServerStatus): string {
  const modelPart = status.loadedModel
    ? `, model ${status.loadedModel.id}`
    : ", no model loaded yet";
  return `Connected — engine ${status.engine}, GPU offload: ${status.gpuOffload}${modelPart}.`;
}

/**
 * "Test connection" for the "Custom" STT provider pointed at a self-hosted
 * companion server (docs/stt-server-design.md, issue #14 Phase 5). Hits
 * `<base>/api/status` and surfaces engine + GPU offload + the currently
 * loaded model on success, or a clear failure otherwise.
 */
export function TestConnectionButton({
  baseUrl,
  apiKey,
}: {
  baseUrl: string;
  apiKey: string;
}) {
  const { t } = useLingui();
  const [state, setState] = useState<TestState>({ kind: "idle" });

  const handleClick = async () => {
    setState({ kind: "pending" });
    const result = await fetchSttServerStatus(baseUrl, apiKey);
    setState(
      result.ok
        ? { kind: "success", status: result.status }
        : { kind: "error", message: result.error },
    );
  };

  return (
    <div className="flex flex-col gap-2" data-testid="stt-test-connection">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="w-fit gap-1.5"
        disabled={state.kind === "pending" || !baseUrl.trim()}
        onClick={handleClick}
      >
        {state.kind === "pending" && (
          <Loader2 size={14} className="animate-spin" />
        )}
        <Trans>Test connection</Trans>
      </Button>

      {state.kind === "success" && (
        <div className="flex items-start gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
          <CheckCircle2 size={14} className="mt-0.5 shrink-0" />
          <span>{formatSuccessDetail(state.status)}</span>
        </div>
      )}

      {state.kind === "error" && (
        <div className="text-destructive flex items-start gap-1.5 text-xs">
          <XCircle size={14} className="mt-0.5 shrink-0" />
          <span>{state.message || t`Could not connect.`}</span>
        </div>
      )}
    </div>
  );
}
