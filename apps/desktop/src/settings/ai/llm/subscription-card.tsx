import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2,
  KeyRound,
  Loader2,
  LogOut,
  ShieldAlert,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { formatDistanceToNow, safeParseDate } from "@hypr/utils";

import { SubscriptionDisclosureModal } from "./disclosure-modal";
import {
  type Provider,
  type SubscriptionProviderId,
  toLlmAuthProvider,
} from "./shared";

import {
  commands as llmAuthCommands,
  type LlmAuthCatalogProvider,
  type LlmAuthStatus,
} from "~/ai/providers/llm-auth-client";
import { ProviderIconSlot } from "~/settings/ai/shared";
import { useConfigValue } from "~/shared/config";

export function SubscriptionCard({
  provider,
  catalogProvider,
}: {
  provider: Provider;
  catalogProvider: LlmAuthCatalogProvider;
}) {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const pluginProvider = toLlmAuthProvider(
    provider.id as SubscriptionProviderId,
  );

  const disclosureAccepted = useConfigValue(
    "llm_subscription_disclosure_accepted",
  );
  const [disclosureOpen, setDisclosureOpen] = useState(false);

  const [isStarting, setIsStarting] = useState(false);
  const [isWaitingComplete, setIsWaitingComplete] = useState(false);
  const [showPasteInput, setShowPasteInput] = useState(false);
  const [pastedCode, setPastedCode] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const pasteTimeoutRef = useRef<number | undefined>(undefined);
  const completeAbortRef = useRef<boolean>(false);

  const statusQuery = useQuery({
    queryKey: ["llm-auth", "status", pluginProvider],
    queryFn: async (): Promise<LlmAuthStatus> => {
      const res = await llmAuthCommands.status(pluginProvider);
      if (res.status === "ok") {
        return res.data;
      }
      return { signed_in: false, account_label: null, expires_at: null };
    },
    staleTime: 1000 * 10,
  });

  const authStatus = statusQuery.data;
  const isSignedIn = !!authStatus?.signed_in;

  useEffect(() => {
    return () => {
      clearTimeout(pasteTimeoutRef.current);
    };
  }, []);

  const handleStartFlow = useCallback(async () => {
    setIsStarting(true);
    setErrorMessage(null);
    setShowPasteInput(false);
    setPastedCode("");
    completeAbortRef.current = false;

    try {
      const startRes = await llmAuthCommands.start(pluginProvider);
      if (startRes.status !== "ok") {
        setErrorMessage(startRes.error);
        setIsStarting(false);
        return;
      }

      const { authorize_url, mode } = startRes.data;

      // Open in system default browser
      await openerCommands.openUrl(authorize_url, null);

      setIsStarting(false);

      if (mode === "paste") {
        setShowPasteInput(true);
        setIsWaitingComplete(false);
      } else {
        setIsWaitingComplete(true);
        // Mode is loopback: start 60s fallback timer to show paste input
        pasteTimeoutRef.current = window.setTimeout(() => {
          setShowPasteInput(true);
        }, 60_000);

        // Await loopback completion
        const completeRes = await llmAuthCommands.complete(
          pluginProvider,
          null,
        );
        if (completeAbortRef.current) {
          return;
        }

        if (completeRes.status === "ok" && completeRes.data.signed_in) {
          clearTimeout(pasteTimeoutRef.current);
          setIsWaitingComplete(false);
          setShowPasteInput(false);
          queryClient.invalidateQueries({
            queryKey: ["llm-auth", "status", pluginProvider],
          });
        } else if (completeRes.status !== "ok") {
          setShowPasteInput(true);
          setErrorMessage(completeRes.error);
        }
      }
    } catch (err) {
      if (!completeAbortRef.current) {
        setErrorMessage(err instanceof Error ? err.message : String(err));
        setShowPasteInput(true);
      }
    } finally {
      setIsStarting(false);
    }
  }, [pluginProvider, queryClient]);

  const handleSignInClick = useCallback(() => {
    if (!disclosureAccepted) {
      setDisclosureOpen(true);
    } else {
      void handleStartFlow();
    }
  }, [disclosureAccepted, handleStartFlow]);

  const handlePasteSubmit = useCallback(async () => {
    if (!pastedCode.trim()) return;

    setIsWaitingComplete(true);
    setErrorMessage(null);
    try {
      const res = await llmAuthCommands.complete(
        pluginProvider,
        pastedCode.trim(),
      );
      if (res.status === "ok" && res.data.signed_in) {
        clearTimeout(pasteTimeoutRef.current);
        setIsWaitingComplete(false);
        setShowPasteInput(false);
        setPastedCode("");
        queryClient.invalidateQueries({
          queryKey: ["llm-auth", "status", pluginProvider],
        });
      } else {
        setErrorMessage(
          res.status === "error"
            ? res.error
            : t`Verification failed. Check the code and try again.`,
        );
      }
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setIsWaitingComplete(false);
    }
  }, [pastedCode, pluginProvider, queryClient, t]);

  const handleCancel = useCallback(async () => {
    completeAbortRef.current = true;
    clearTimeout(pasteTimeoutRef.current);
    setIsStarting(false);
    setIsWaitingComplete(false);
    setShowPasteInput(false);
    setPastedCode("");
    setErrorMessage(null);
    await llmAuthCommands.cancel(pluginProvider);
  }, [pluginProvider]);

  const handleSignOut = useCallback(async () => {
    await llmAuthCommands.logout(pluginProvider);
    queryClient.invalidateQueries({
      queryKey: ["llm-auth", "status", pluginProvider],
    });
  }, [pluginProvider, queryClient]);

  const expiryText = authStatus?.expires_at
    ? (() => {
        const date = safeParseDate(authStatus.expires_at);
        if (!date) return null;
        return formatDistanceToNow(date, { addSuffix: true });
      })()
    : null;

  const inProgress = isStarting || isWaitingComplete || showPasteInput;

  return (
    <>
      <div className="border-border/60 bg-card rounded-xl border p-4 shadow-xs">
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
              <div className="flex flex-col">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium">
                    {catalogProvider.label || provider.displayName}
                  </span>
                  <Badge
                    variant="outline"
                    className="border-amber-500/30 bg-amber-500/10 text-[10px] text-amber-600 dark:text-amber-400"
                  >
                    <Trans>Unofficial</Trans>
                  </Badge>
                </div>
                {catalogProvider.notes && (
                  <span className="text-muted-foreground text-xs">
                    {catalogProvider.notes}
                  </span>
                )}
              </div>
            </div>

            {/* Status Chip / Action Buttons */}
            <div className="flex items-center gap-2">
              {isSignedIn ? (
                <div className="flex items-center gap-2">
                  <div className="bg-ok/10 text-ok border-ok/20 flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium">
                    <CheckCircle2 className="size-3.5" />
                    <span>{authStatus.account_label || t`Signed in`}</span>
                    {expiryText && (
                      <span className="text-muted-foreground text-[10px]">
                        ({expiryText})
                      </span>
                    )}
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleSignOut}
                    className="text-muted-foreground hover:text-foreground h-8 text-xs"
                  >
                    <LogOut className="mr-1 size-3.5" />
                    <Trans>Sign out</Trans>
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <div className="bg-muted text-muted-foreground flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs">
                    <KeyRound className="size-3" />
                    <Trans>Not connected</Trans>
                  </div>
                  {!inProgress && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleSignInClick}
                      className="h-8 text-xs"
                    >
                      <Trans>Sign in</Trans>
                    </Button>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Active Sign-in In-Flight Panel */}
          {inProgress && (
            <div className="border-border/40 bg-muted/40 flex flex-col gap-3 rounded-lg border p-3">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 text-xs">
                  <Loader2 className="text-primary size-4 animate-spin" />
                  <span>
                    <Trans>Signing in with your browser…</Trans>
                  </span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleCancel}
                  className="h-7 text-xs"
                >
                  <Trans>Cancel</Trans>
                </Button>
              </div>

              {/* Paste code input */}
              {showPasteInput && (
                <div className="flex flex-col gap-2 pt-1">
                  <p className="text-muted-foreground text-xs">
                    <Trans>
                      If the automatic callback did not complete, paste the
                      authorization code from your browser below:
                    </Trans>
                  </p>
                  <div className="flex items-center gap-2">
                    <Input
                      placeholder={t`Paste authorization code here`}
                      value={pastedCode}
                      onChange={(e) => setPastedCode(e.target.value)}
                      className="h-8 font-mono text-xs"
                    />
                    <Button
                      size="sm"
                      onClick={handlePasteSubmit}
                      disabled={!pastedCode.trim() || isWaitingComplete}
                      className="h-8 text-xs"
                    >
                      <Trans>Complete</Trans>
                    </Button>
                  </div>
                </div>
              )}

              {errorMessage && (
                <div className="flex items-center gap-1.5 text-xs text-red-500">
                  <ShieldAlert className="size-3.5 shrink-0" />
                  <span>{errorMessage}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <SubscriptionDisclosureModal
        open={disclosureOpen}
        onOpenChange={setDisclosureOpen}
        onAccept={() => void handleStartFlow()}
      />
    </>
  );
}
