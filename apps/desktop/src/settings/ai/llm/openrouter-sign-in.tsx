import { Trans } from "@lingui/react/macro";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useRef } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { PROVIDERS } from "./shared";

import { commands as llmAuthCommands } from "~/ai/providers/llm-auth-client";
import {
  loadSecureAiProviderApiKeys,
  setAiProvider,
  useAiProvider,
} from "~/settings/providers";

const OPENROUTER_ROW_ID = "llm:openrouter";

export function OpenRouterSignIn({
  setApiKey,
}: {
  setApiKey: (apiKey: string) => void;
}) {
  const queryClient = useQueryClient();
  const current = useAiProvider("llm", "openrouter");
  const cancelledRef = useRef(false);

  const signIn = useMutation({
    mutationFn: async () => {
      cancelledRef.current = false;
      const res = await llmAuthCommands.openrouterSignIn();
      if (res.status === "error") {
        if (cancelledRef.current) return;
        throw new Error(res.error);
      }

      // The Rust command already stored the key in the same secure-store slot
      // the manual API key field uses. Make sure the provider row exists and
      // read the key back so the card's form does not overwrite it with its
      // stale empty value.
      const keys = await loadSecureAiProviderApiKeys(["llm:openrouter"], "llm");
      const apiKey = keys[OPENROUTER_ROW_ID] ?? "";
      await setAiProvider("llm", "openrouter", {
        base_url:
          current?.base_url ||
          PROVIDERS.find((p) => p.id === "openrouter")?.baseUrl ||
          "",
        api_key: apiKey,
      });
      await queryClient.invalidateQueries({
        queryKey: ["ai-provider-api-keys", "llm"],
      });
      setApiKey(apiKey);
    },
  });

  const cancel = () => {
    cancelledRef.current = true;
    void llmAuthCommands.openrouterCancel();
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        {signIn.isPending ? (
          <>
            <Loader2 className="text-primary size-4 animate-spin" />
            <span className="text-xs">
              <Trans>Waiting for OpenRouter in your browser…</Trans>
            </span>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 text-xs"
              onClick={cancel}
            >
              <Trans>Cancel</Trans>
            </Button>
          </>
        ) : (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-8 text-xs"
            onClick={() => signIn.mutate()}
          >
            <Trans>Sign in with OpenRouter</Trans>
          </Button>
        )}
      </div>
      {signIn.isError && (
        <p className="text-destructive text-xs">{signIn.error.message}</p>
      )}
      <p className="text-muted-foreground text-xs">
        <Trans>Or enter an API key manually below.</Trans>
      </p>
    </div>
  );
}
