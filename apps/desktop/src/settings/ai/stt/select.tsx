import { Trans, useLingui } from "@lingui/react/macro";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { arch } from "@tauri-apps/plugin-os";
import {
  AlertTriangle,
  Check,
  DownloadIcon,
  FolderOpen,
  Loader2,
  Trash2,
  Zap,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  commands as localSttCommands,
  type LocalModel,
  type SttModelLanguages,
  type SttModelTier,
  type SttRecommendedUse,
} from "@hypr/plugin-local-stt";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import type { AIProviderStorage } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useSttSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import {
  activateCustomSttModel,
  type CustomSttModel,
  downloadCustomSttModel,
  fetchCustomSttModelProgress,
  listCustomSttModels,
} from "./list-custom-stt";
import { LocalModelBackendBadge, LocalModelLabel } from "./model-icon";
import { resolveLiveLanguageSupportMode } from "./selection";
import {
  displayModelLabel,
  formatModelSize,
  modelBadgeAccentClassName,
  modelBadgeNeutralClassName,
  type ProviderId,
  PROVIDERS,
  SttLanguageBadge,
  SttModelUseHint,
  sttModelQueries,
  SttTierBadge,
} from "./shared";

import { useBillingAccess } from "~/auth/billing";
import { useNotifications } from "~/contexts/notifications";
import { providerRowId, ProviderIconSlot } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { useAiProviders } from "~/settings/providers";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { SettingsAlert } from "~/shared/ui/settings-alert";
import {
  showTransientToast,
  useTransientToast,
} from "~/sidebar/toast/transient";
import {
  isConfiguredSttModel,
  isHyprnoteLocalSttModel,
  isLiveTranscriptionSupported,
  isParakeetLocalSttModel,
  isRealtimeLocalModel,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
  isSupportedLocalSttModel,
  isWhisperLocalSttModel,
} from "~/stt/capabilities";
import {
  getDefaultSttModel,
  getPreferredProviderModel,
} from "~/stt/model-selection";

// A local on-device model that can transcribe live (whisper.cpp / parakeet ONNX
// stream final transcripts over the loopback /v1/listen websocket; the macOS
// soniqo streaming model is realtime). Voxtral/am/soniqo-batch are batch-only,
// so they are NOT offered in the live picker — selecting one would silently
// degrade live to batch (the regression this split fixes).
function isLiveCapableLocalModel(modelId: string) {
  return (
    isWhisperLocalSttModel(modelId) ||
    isParakeetLocalSttModel(modelId) ||
    isRealtimeLocalModel(modelId)
  );
}

export function SelectLiveModel() {
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();

  const handleSelectProvider = useSetSettingValue("current_stt_provider");
  const handleSelectModel = useSetSettingValue("current_stt_model");

  const localModels = configuredProviders.hyprnote?.models ?? [];
  const liveModels = localModels.filter((model) =>
    isLiveCapableLocalModel(model.id),
  );

  const isLocalActive = isHyprnoteLocalSttModel(
    current_stt_provider,
    current_stt_model,
  );
  // Remote/custom/cloud active selections can't transcribe live; surface that
  // as an explicit disabled state instead of silently no-op'ing the orb and
  // meeting-live start. The list below still lets the user pick a local model
  // to enable live.
  const liveDisabled = !isLocalActive;
  const selectedModelEntry = isLocalActive
    ? liveModels.find((model) => model.id === current_stt_model)
    : undefined;
  const isConfigured = isLocalActive && !!selectedModelEntry;
  const hasError = isConfigured && health.status === "error";

  const handleModelChange = (model: string) => {
    // Selecting a local model makes the live connection local; this also
    // starts the loopback STT server via syncLocalSttServer.
    handleSelectProvider("hyprnote");
    handleSelectModel(model);
  };

  return (
    <div className="flex flex-col gap-4">
      {liveDisabled && (
        <SettingsAlert>
          <Trans>
            <strong className="font-medium">Live transcription</strong> needs a
            downloaded on-device model. Remote, custom, and cloud providers
            can't transcribe live — pick one below, or use them for the batch
            pass instead.
          </Trans>
        </SettingsAlert>
      )}

      {hasError && health.message && (
        <SettingsAlert>{health.message}</SettingsAlert>
      )}

      <div className="flex items-center gap-2">
        <h3 className="text-md font-sans font-semibold">
          <Trans>Live transcription model</Trans>
        </h3>
        <HealthStatusIndicator />
        {isConfigured && health.status === "success" && (
          <span
            data-testid="stt-live-connected"
            className="text-ok flex items-center gap-1 text-[11px] font-medium"
          >
            <Check className="size-3.5 shrink-0" />
            <Trans>Connected</Trans>
          </span>
        )}
      </div>
      <p className="text-muted-foreground -mt-2 text-sm">
        <Trans>
          Transcribes while the meeting is happening and powers the dictation
          orb. Only downloaded on-device models can listen live.
        </Trans>
      </p>

      <ModelRowList
        testId="stt-live-model-list"
        models={liveModels}
        selectedId={isLocalActive ? current_stt_model : undefined}
        pairing="live"
        onSelect={handleModelChange}
        onDownload={(model) => startDownload(model as LocalModel)}
        onStartTrial={startTrial}
      />
    </div>
  );
}

// Sentinel Select value meaning "no dedicated batch provider" — batch falls
// back to the live model (Radix Select items can't use an empty-string value).
// Stored as "" in both final_stt_provider and final_stt_model.
const FINAL_PROVIDER_SAME_AS_LIVE = "__same_as_live__";

export function SelectBatchModel() {
  const { t } = useLingui();
  const { final_stt_provider, final_stt_model } = useConfigValues([
    "final_stt_provider",
    "final_stt_model",
  ] as const);
  const billing = useBillingAccess();
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const handleSelectFinalProvider = useSetSettingValue("final_stt_provider");
  const handleSelectFinalModel = useSetSettingValue("final_stt_model");

  const selectedProviderValue =
    typeof final_stt_provider === "string" ? final_stt_provider.trim() : "";
  const selectedProvider = selectedProviderValue as ProviderId | "";
  const providerModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];
  const finalModel = final_stt_model?.trim() ?? "";
  const selectedFinalModel = providerModels.find(
    (model) => model.id === finalModel,
  );
  const customConfig = configuredProviders.custom;

  const handleProviderChange = (provider: string) => {
    if (provider === FINAL_PROVIDER_SAME_AS_LIVE) {
      handleSelectFinalProvider("");
      handleSelectFinalModel("");
      return;
    }

    const providerId = provider as ProviderId;
    const nextModels = configuredProviders[providerId]?.models ?? [];
    const nextModel =
      getPreferredProviderModel(undefined, nextModels) ||
      getDefaultSttModel(providerId) ||
      (providerId === "custom" ? "" : "");

    handleSelectFinalProvider(provider);
    handleSelectFinalModel(nextModel);
  };

  const handleModelChange = (model: string) => {
    handleSelectFinalModel(model);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <h3 className="text-md font-sans font-semibold">
          <Trans>Batch & final-pass model</Trans>
        </h3>
      </div>
      <p className="text-muted-foreground -mt-2 text-sm">
        <Trans>
          Used for the accurate pass after a recording ends, imported files, and
          re-transcription. Pick a larger on-device model, a custom self-hosted
          server, or a cloud provider — independent of the live model.
        </Trans>
      </p>

      <div className="max-w-md" data-stt-batch-provider-selector>
        <Select
          value={selectedProvider || FINAL_PROVIDER_SAME_AS_LIVE}
          onValueChange={handleProviderChange}
        >
          <SelectTrigger className="bg-card shadow-none focus:ring-0">
            <SelectValue placeholder={t`Select a provider`} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={FINAL_PROVIDER_SAME_AS_LIVE}>
              <div className="flex flex-col gap-0.5">
                <span>
                  <Trans>Same as live model</Trans>
                </span>
                <span className="text-muted-foreground text-[11px]">
                  <Trans>Reuses the live model for batch transcription.</Trans>
                </span>
              </div>
            </SelectItem>
            {PROVIDERS.filter(({ disabled }) => !disabled).map((provider) => {
              const configured =
                configuredProviders[provider.id]?.configured ?? false;
              const requiresPro = requiresEntitlement(
                provider.requirements,
                "pro",
              );
              const locked = requiresPro && !billing.isPaid;
              return (
                <SelectItem
                  key={provider.id}
                  value={provider.id}
                  disabled={provider.disabled || locked}
                  className={cn([
                    "data-disabled:text-muted-foreground data-disabled:!opacity-100",
                    !configured && !locked && "text-muted-foreground",
                  ])}
                >
                  <div className="flex flex-col gap-0.5">
                    <div className="flex items-center gap-2">
                      <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                      <span>{provider.displayName}</span>
                      {requiresPro ? (
                        <span className="border-border text-muted-foreground rounded-full border px-2 py-0.5 text-[10px] tracking-wide uppercase">
                          <Trans>Pro</Trans>
                        </span>
                      ) : null}
                    </div>
                    {locked ? (
                      <span className="text-muted-foreground text-[11px]">
                        <Trans>Upgrade to Pro to use this provider.</Trans>
                      </span>
                    ) : null}
                  </div>
                </SelectItem>
              );
            })}
          </SelectContent>
        </Select>
      </div>

      {selectedProvider === "custom" ? (
        <CustomSttModelSection
          baseUrl={customConfig?.baseUrl ?? ""}
          apiKey={customConfig?.apiKey ?? ""}
          selectedId={finalModel}
          onSelect={handleModelChange}
        />
      ) : selectedProvider ? (
        <ModelRowList
          testId="stt-batch-model-list"
          models={providerModels}
          selectedId={selectedFinalModel ? selectedFinalModel.id : undefined}
          pairing="final"
          onSelect={handleModelChange}
          onDownload={(model) => startDownload(model as LocalModel)}
          onStartTrial={startTrial}
        />
      ) : (
        <p className="text-muted-foreground text-[13px]">
          <Trans>
            Batch transcription uses the live model. Pick a provider above to
            run the final pass on a different model.
          </Trans>
        </p>
      )}
    </div>
  );
}

/**
 * Model picker for the "Custom" STT provider: lists the models the connected
 * self-hosted Notare STT server exposes (GET <origin>/api/models), shows
 * which are active/installed, lets the user download a not-installed model
 * (POST .../download, polled via .../progress) and activate one
 * (POST .../activate). Selecting a row stores the model id in
 * `final_stt_model`, exactly what the free-text input did. A free-text input
 * stays available below so an unreachable server or an unlisted id never
 * regresses the manual workflow.
 */
export function CustomSttModelSection({
  baseUrl,
  apiKey,
  selectedId,
  onSelect,
}: {
  baseUrl: string;
  apiKey: string;
  selectedId: string;
  onSelect: (id: string) => void;
}) {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const trimmedBaseUrl = baseUrl.trim();
  const queryKey = ["stt-custom-models", trimmedBaseUrl, apiKey.trim()];

  const modelsQuery = useQuery({
    queryKey,
    queryFn: ({ signal }) =>
      listCustomSttModels(trimmedBaseUrl, apiKey, signal),
    enabled: trimmedBaseUrl.length > 0,
    staleTime: 1000 * 5,
    retry: false,
  });

  const models = modelsQuery.data?.ok ? modelsQuery.data.models : [];
  const fetchError =
    modelsQuery.data && !modelsQuery.data.ok ? modelsQuery.data.error : null;
  const hasModels = models.length > 0;

  // If the chosen id isn't one the server exposes (server down, unlisted, or
  // a value typed manually into the fallback), we still show it selected so
  // the user can see what's stored.
  const selectedEntry = models.find((model) => model.id === selectedId);

  return (
    <div className="flex flex-col gap-2">
      {hasModels ? (
        <div
          role="radiogroup"
          data-testid="stt-custom-model-list"
          className="border-border divide-border bg-card divide-y overflow-hidden rounded-[10px] border"
        >
          {models.map((model) => (
            <CustomSttModelRow
              key={model.id}
              model={model}
              baseUrl={trimmedBaseUrl}
              apiKey={apiKey}
              selected={model.id === selectedId}
              onSelect={() => onSelect(model.id)}
              onDownloaded={() => void modelsQuery.refetch()}
              onActivated={() => {
                void queryClient.invalidateQueries({ queryKey });
                onSelect(model.id);
              }}
            />
          ))}
        </div>
      ) : (
        <p className="text-muted-foreground text-[13px]">
          {modelsQuery.isLoading
            ? t`Loading models...`
            : fetchError
              ? fetchError
              : t`No models are available for this provider.`}
        </p>
      )}

      {selectedEntry ? null : (
        <Input
          value={selectedId}
          onChange={(event) => onSelect(event.target.value)}
          className="text-xs"
          placeholder={t`Enter a model identifier`}
          aria-label={t`Custom model identifier`}
        />
      )}
    </div>
  );
}

// After this many consecutive null/failed progress polls, stop polling and
// surface a "download stalled — retry" state instead of spinning forever.
export const PROGRESS_MAX_CONSECUTIVE_FAILURES = 5;

function CustomSttModelRow({
  model,
  baseUrl,
  apiKey,
  selected,
  onSelect,
  onDownloaded,
  onActivated,
}: {
  model: CustomSttModel;
  baseUrl: string;
  apiKey: string;
  selected: boolean;
  onSelect: () => void;
  onDownloaded: () => void;
  onActivated: () => void;
}) {
  const { t } = useLingui();
  const [downloadPercent, setDownloadPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activating, setActivating] = useState(false);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isDownloading = downloadPercent !== null;
  const sizeLabel = formatModelSize(model.sizeBytes);

  useEffect(() => {
    return () => {
      if (pollRef.current) {
        clearTimeout(pollRef.current);
      }
    };
  }, []);

  const stopPolling = () => {
    if (pollRef.current) {
      clearTimeout(pollRef.current);
      pollRef.current = null;
    }
  };

  const pollProgress = (consecutiveFailures = 0) => {
    stopPolling();
    pollRef.current = setTimeout(async () => {
      const progress = await fetchCustomSttModelProgress(
        baseUrl,
        apiKey,
        model.id,
      );
      if (!progress) {
        // Can't read progress — the download may still be running server-side,
        // so retry. But bound it: after N consecutive failed polls the server
        // is likely gone, so stop spinning forever and surface a retry state
        // via the existing error display instead of an infinite spinner.
        if (consecutiveFailures + 1 >= PROGRESS_MAX_CONSECUTIVE_FAILURES) {
          setDownloadPercent(null);
          setError(t`Download stalled — retry.`);
          return;
        }
        pollProgress(consecutiveFailures + 1);
        return;
      }

      if (progress.failed) {
        setDownloadPercent(null);
        setError(progress.detail ?? t`Download failed.`);
        return;
      }

      if (progress.complete) {
        setDownloadPercent(null);
        onDownloaded();
        return;
      }

      setDownloadPercent(progress.percent ?? downloadPercent);
      // A successful poll resets the failure streak.
      pollProgress(0);
    }, 1500);
  };

  const handleDownload = async () => {
    setError(null);
    setDownloadPercent(0);
    const result = await downloadCustomSttModel(baseUrl, apiKey, model.id);
    if (!result.ok) {
      setDownloadPercent(null);
      setError(result.error);
      return;
    }
    if (result.alreadyInstalled) {
      setDownloadPercent(null);
      onDownloaded();
      return;
    }
    pollProgress();
  };

  const handleActivate = async () => {
    setError(null);
    setActivating(true);
    const result = await activateCustomSttModel(baseUrl, apiKey, model.id);
    if (!result.ok) {
      setActivating(false);
      setError(result.error);
      return;
    }
    // A 200 only means the server accepted the request, not that the model is
    // actually active. Refetch and confirm the server reports this model as
    // active before treating it as such; otherwise warn and refresh the list
    // so the row reflects the real state instead of a false "Active".
    const verified = await listCustomSttModels(baseUrl, apiKey);
    setActivating(false);
    if (!verified.ok) {
      onDownloaded();
      setError(t`Activated, but the server didn't confirm it as active.`);
      return;
    }
    const confirmed = verified.models.some(
      (m) => m.id === model.id && m.active,
    );
    if (!confirmed) {
      onDownloaded();
      setError(t`Activated, but the server didn't confirm it as active.`);
      return;
    }
    onActivated();
  };

  const handleClick = () => {
    if (model.installed && !model.corrupt) {
      onSelect();
    }
  };

  const selectable = model.installed && !model.corrupt;

  return (
    <div
      role="radio"
      aria-checked={selected}
      tabIndex={selectable ? 0 : -1}
      onClick={selectable ? handleClick : undefined}
      onKeyDown={
        selectable
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                handleClick();
              }
            }
          : undefined
      }
      className={cn([
        "group/custom-model-row relative flex items-center gap-2.5 px-3 py-2 text-left outline-hidden",
        "transition-colors duration-(--motion-duration-state)",
        selectable && "hover:bg-accent/50 cursor-pointer",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:ring-inset",
        selected && "bg-primary/5",
      ])}
    >
      <span
        aria-hidden
        className={cn([
          "mt-1 size-3.5 shrink-0 rounded-full border transition-colors duration-(--motion-duration-state)",
          selected
            ? "border-primary border-[4.5px]"
            : selectable
              ? "border-border group-hover/custom-model-row:border-muted-foreground"
              : "border-border border-dashed",
        ])}
      />
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <span
            className={cn([
              "min-w-0 text-[13px] font-medium",
              !model.installed && "text-muted-foreground",
            ])}
          >
            {model.displayName}
          </span>
          <CustomSttModelStatusBadge model={model} />
          {model.englishOnly ? (
            <span className="text-muted-foreground shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium">
              EN
            </span>
          ) : null}
          {sizeLabel ? (
            <span className="text-muted-foreground shrink-0 font-mono text-[11px]">
              {sizeLabel}
            </span>
          ) : null}
        </div>
        {model.description ? (
          <span className="text-muted-foreground mt-0.5 block min-w-0 truncate text-[11px]">
            {model.description}
          </span>
        ) : null}
        {error ? (
          <span className="text-destructive mt-0.5 block text-[11px]">
            {error}
          </span>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center gap-1 self-center">
        {isDownloading ? (
          <span
            className={cn([
              "flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium",
              "border-primary/25 bg-primary/10 text-primary",
            ])}
          >
            <Loader2 className="size-3 animate-spin" />
            <span className="font-mono">
              {downloadPercent !== null ? `${downloadPercent}%` : "…"}
            </span>
          </span>
        ) : model.installed && !model.active ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-6 gap-1 px-2 text-[11px]"
            disabled={activating}
            onClick={(event) => {
              event.stopPropagation();
              void handleActivate();
            }}
          >
            {activating ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Zap className="size-3" />
            )}
            <Trans>Activate</Trans>
          </Button>
        ) : !model.unknown && (!model.installed || model.corrupt) ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-6 gap-1 px-2 text-[11px]"
            onClick={(event) => {
              event.stopPropagation();
              void handleDownload();
            }}
          >
            <DownloadIcon className="size-3" />
            <Trans>Download</Trans>
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function CustomSttModelStatusBadge({ model }: { model: CustomSttModel }) {
  if (model.active) {
    return (
      <span
        className={cn([
          "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
          "border-primary/25 bg-primary/10 text-primary",
        ])}
      >
        <Trans>Active</Trans>
      </span>
    );
  }

  if (model.corrupt) {
    return (
      <span
        className={cn([
          "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
          "border-alert-foreground/30 bg-alert-foreground/10 text-alert-foreground",
        ])}
      >
        <Trans>Corrupt</Trans>
      </span>
    );
  }

  if (model.installed) {
    return (
      <span
        className={cn([
          "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
          modelBadgeNeutralClassName,
        ])}
      >
        <Trans>Installed</Trans>
      </span>
    );
  }

  if (model.unknown) {
    return (
      <span
        className={cn([
          "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
          modelBadgeNeutralClassName,
        ])}
      >
        <Trans>Unknown</Trans>
      </span>
    );
  }

  return (
    <span
      className={cn([
        "shrink-0 rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
        modelBadgeNeutralClassName,
      ])}
    >
      <Trans>Not installed</Trans>
    </span>
  );
}

const TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID =
  "transcription-language-warning";
const dismissedTranscriptionLanguageWarningKeys = new Set<string>();

export function TranscriptionLanguageWarningToast() {
  const warningKey = useTranscriptionLanguageWarningKey();

  if (
    !warningKey ||
    dismissedTranscriptionLanguageWarningKeys.has(warningKey)
  ) {
    return null;
  }

  return (
    <TranscriptionLanguageWarningToastLifecycle
      key={warningKey}
      warningKey={warningKey}
    />
  );
}

function TranscriptionLanguageWarningToastLifecycle({
  warningKey,
}: {
  warningKey: string;
}) {
  useMountEffect(() => {
    showTransientToast(
      {
        id: TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID,
        icon: (
          <AlertTriangle className="text-alert-foreground size-4 shrink-0" />
        ),
        description: "Model doesn't support all languages.",
        anchor: "main-content-panel",
        actions: [
          {
            label: "Dismiss",
            onClick: () => {
              dismissedTranscriptionLanguageWarningKeys.add(warningKey);
              clearTranscriptionLanguageWarningToast();
            },
          },
        ],
        dismissible: false,
        variant: "warning",
      },
      { durationMs: null },
    );

    return clearTranscriptionLanguageWarningToast;
  });

  return null;
}

function clearTranscriptionLanguageWarningToast() {
  const { toast, clearToast } = useTransientToast.getState();

  if (toast?.id === TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID) {
    clearToast(toast.key);
  }
}

function useTranscriptionLanguageWarningKey() {
  const { current_stt_provider, current_stt_model, spoken_languages } =
    useConfigValues([
      "current_stt_provider",
      "current_stt_model",
      "spoken_languages",
    ] as const);
  const health = useConnectionHealth();

  const selectedSttModel = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : undefined;
  const isConfigured = !!(current_stt_provider && selectedSttModel);
  const isOnDeviceModel = isHyprnoteLocalSttModel(
    current_stt_provider,
    selectedSttModel,
  );
  const useLiveOnDeviceModel =
    isOnDeviceModel && isRealtimeLocalModel(selectedSttModel);
  const hasError = isConfigured && health.status === "error";
  const liveSupport = useQuery({
    queryKey: ["stt-live-support", current_stt_provider, selectedSttModel],
    queryFn: () =>
      isLiveTranscriptionSupported(current_stt_provider, selectedSttModel),
    enabled: isConfigured,
  });
  const useLiveMode = resolveLiveLanguageSupportMode({
    isOnDeviceModel,
    useLiveOnDeviceModel,
    liveSupported: liveSupport.data,
  });

  const languageSupport = useQuery({
    queryKey: [
      "stt-language-support",
      current_stt_provider,
      selectedSttModel,
      useLiveMode,
      spoken_languages,
    ],
    queryFn: async () =>
      useLiveMode
        ? await isSupportedLanguagesLive(
            current_stt_provider!,
            selectedSttModel ?? null,
            spoken_languages ?? [],
          )
        : await isSupportedLanguagesBatch(
            current_stt_provider!,
            selectedSttModel ?? null,
            spoken_languages ?? [],
          ),
    enabled:
      isConfigured &&
      liveSupport.data !== undefined &&
      !!spoken_languages?.length,
  });

  if (!isConfigured || languageSupport.data !== false || hasError) {
    return null;
  }

  return [
    current_stt_provider,
    selectedSttModel,
    ...(spoken_languages ?? []),
  ].join(":");
}

type ModelCategory = "latest" | null;
type ModelEntry = {
  id: string;
  isDownloaded: boolean;
  displayName?: string;
  isDeprecated?: boolean;
  category?: ModelCategory;
  sizeBytes?: number | null;
  mode?: "realtime" | "batch";
  engine?: string;
  languages?: SttModelLanguages;
  languageCount?: number | null;
  tier?: SttModelTier;
  recommendedUse?: SttRecommendedUse;
};

// The "On device" placeholder label hides which model a row is; local rows
// show the actual model name instead, with the engine as the hover title.
function modelEntryLabel(model: ModelEntry) {
  return model.displayName ?? displayModelLabel(model.id);
}

function getModelCategoryLabel(category?: ModelCategory) {
  if (category === "latest") {
    return "Recommended";
  }

  return null;
}

function getProviderModelMode(
  providerId: ProviderId,
  model: string,
): ModelEntry["mode"] {
  if (providerId === "assemblyai") {
    if (model === "universal-3-pro") {
      return "batch";
    }

    if (model === "u3-rt-pro") {
      return "realtime";
    }
  }

  if (providerId === "elevenlabs") {
    if (model === "scribe_v2") {
      return "batch";
    }

    if (model === "scribe_v2_realtime") {
      return "realtime";
    }
  }

  if (providerId === "mistral") {
    if (model === "voxtral-mini-2602" || model === "voxtral-mini-latest") {
      return "batch";
    }

    if (model === "voxtral-mini-transcribe-realtime-2602") {
      return "realtime";
    }
  }

  if (providerId === "soniox") {
    if (model === "stt-async-v5" || model === "stt-async-v4") {
      return "batch";
    }

    if (
      model === "stt-rt-v5" ||
      model === "stt-rt-v4" ||
      model === "stt-v5" ||
      model === "stt-v4"
    ) {
      return "realtime";
    }
  }

  return undefined;
}

function useConfiguredMapping(): Record<
  ProviderId,
  {
    configured: boolean;
    models: ModelEntry[];
    baseUrl: string;
    apiKey: string;
  }
> {
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders("stt");

  const targetArch = useQuery({
    queryKey: ["target-arch"],
    queryFn: () => arch(),
    staleTime: Infinity,
  });

  const isAppleSilicon = targetArch.data === "aarch64";

  const supportedModels = useQuery({
    queryKey: ["list-supported-models"],
    queryFn: async () => {
      const result = await localSttCommands.listSupportedModels();
      return result.status === "ok" ? result.data : [];
    },
    staleTime: Infinity,
  });

  const localModels = supportedModels.data ?? [];
  // whisper.cpp, Parakeet (ONNX) and Voxtral (llama.cpp) models run
  // everywhere the backend compiled them in (the backend's own
  // `list_supported_models` already gates each on its Cargo feature);
  // soniqo/argmax backends are Apple Silicon only.
  const visibleLocalModels = localModels.filter(
    (m) =>
      m.model_type === "whispercpp" ||
      m.model_type === "parakeetOnnx" ||
      m.model_type === "voxtralLlama" ||
      isAppleSilicon,
  );

  const localModelsDownloaded = useQueries({
    queries: [
      ...visibleLocalModels.map((m) => sttModelQueries.isDownloaded(m.key)),
    ],
  });

  return Object.fromEntries(
    PROVIDERS.map((provider) => {
      const config = configuredProviders[providerRowId("stt", provider.id)] as
        | AIProviderStorage
        | undefined;
      const baseUrl = String(config?.base_url || provider.baseUrl || "").trim();
      const apiKey = String(config?.api_key || "").trim();

      const eligible =
        getProviderSelectionBlockers(provider.requirements, {
          isAuthenticated: true,
          isPaid: billing.isPaid,
          config: { base_url: baseUrl, api_key: apiKey },
        }).length === 0;

      if (!eligible) {
        return [
          provider.id,
          { configured: false, models: [], baseUrl, apiKey },
        ];
      }

      if (provider.id === "hyprnote") {
        // No cloud tier in Notare: only on-device models are offered.
        const models: ModelEntry[] = visibleLocalModels.map((model, i) => ({
          id: model.key,
          isDownloaded: localModelsDownloaded[i]?.data ?? false,
          displayName: model.display_name,
          sizeBytes: model.size_bytes,
          mode: isRealtimeLocalModel(String(model.key)) ? "realtime" : "batch",
          category: "latest" as const,
          engine: model.engine,
          languages: model.languages,
          languageCount: model.language_count,
          tier: model.tier,
          recommendedUse: model.recommended_use,
        }));

        return [provider.id, { configured: true, models, baseUrl, apiKey }];
      }

      if (provider.id === "custom") {
        return [provider.id, { configured: true, models: [], baseUrl, apiKey }];
      }

      return [
        provider.id,
        {
          configured: true,
          baseUrl,
          apiKey,
          models: provider.models.map((model) => ({
            id: model,
            isDownloaded: true,
            mode: getProviderModelMode(provider.id, model),
          })),
        },
      ];
    }),
  ) as Record<
    ProviderId,
    {
      configured: boolean;
      models: ModelEntry[];
      baseUrl: string;
      apiKey: string;
    }
  >;
}

/**
 * Always-visible model list (docs/DESIGN-DIRECTION.md §4.6): rows with the
 * full identity of each model — name, engine, language/tier/backend/mode
 * badges and the recommended-use hint — instead of a dropdown that hid them.
 */
function ModelRowList({
  models,
  selectedId,
  pairing,
  onSelect,
  onDownload,
  onStartTrial,
  leadingRow,
  testId,
}: {
  models: ModelEntry[];
  selectedId?: string;
  pairing: "live" | "final";
  onSelect: (id: string) => void;
  onDownload: (id: string) => void;
  onStartTrial: () => void;
  leadingRow?: {
    id: string;
    label: React.ReactNode;
    hint: React.ReactNode;
  };
  testId?: string;
}) {
  if (models.length === 0 && !leadingRow) {
    return (
      <p className="text-muted-foreground text-[13px]">
        <Trans>No models are available for this provider.</Trans>
      </p>
    );
  }

  return (
    <div
      role="radiogroup"
      data-testid={testId}
      className="border-border divide-border bg-card divide-y overflow-hidden rounded-[10px] border"
    >
      {leadingRow ? (
        <ModelRowShell
          selected={selectedId === leadingRow.id}
          selectable
          onActivate={() => onSelect(leadingRow.id)}
        >
          <div className="min-w-0 flex-1">
            <span className="text-[13px] font-medium">{leadingRow.label}</span>
            <span className="text-muted-foreground mt-0.5 block min-w-0 truncate text-[11px]">
              {leadingRow.hint}
            </span>
          </div>
        </ModelRowShell>
      ) : null}
      {models.map((model, i) => {
        const prevCategory = i > 0 ? models[i - 1].category : null;
        const showHeader = model.category && model.category !== prevCategory;
        const categoryLabel = showHeader
          ? getModelCategoryLabel(model.category)
          : null;

        return (
          <div key={model.id} className="divide-border divide-y">
            {categoryLabel && (
              <div className="text-muted-foreground bg-background/40 px-3 pt-2 pb-1 text-[10px] font-medium tracking-wide uppercase">
                {categoryLabel}
              </div>
            )}
            <ModelRow
              model={model}
              selected={selectedId === model.id}
              pairing={pairing}
              onSelect={() => onSelect(model.id)}
              onDownload={() => onDownload(model.id)}
              onStartTrial={onStartTrial}
            />
          </div>
        );
      })}
    </div>
  );
}

function ModelRowShell({
  selected,
  selectable,
  onActivate,
  children,
}: {
  selected: boolean;
  selectable: boolean;
  onActivate: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      role="radio"
      aria-checked={selected}
      tabIndex={selectable ? 0 : -1}
      onClick={selectable ? onActivate : undefined}
      onKeyDown={
        selectable
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onActivate();
              }
            }
          : undefined
      }
      className={cn([
        "group/model-row relative flex items-start gap-2.5 px-3 py-2 text-left outline-hidden",
        "transition-colors duration-(--motion-duration-state)",
        selectable && "hover:bg-accent/50 cursor-pointer",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:ring-inset",
        selected && "bg-primary/5",
      ])}
    >
      <span
        aria-hidden
        className={cn([
          "mt-1 size-3.5 shrink-0 rounded-full border transition-colors duration-(--motion-duration-state)",
          selected
            ? "border-primary border-[4.5px]"
            : selectable
              ? "border-border group-hover/model-row:border-muted-foreground"
              : "border-border border-dashed",
        ])}
      />
      {children}
    </div>
  );
}

function ModelRow({
  model,
  selected,
  pairing,
  onSelect,
  onDownload,
  onStartTrial,
}: {
  model: ModelEntry;
  selected: boolean;
  pairing: "live" | "final";
  onSelect: () => void;
  onDownload: () => void;
  onStartTrial: () => void;
}) {
  const isCloud = model.id === "cloud";
  const { activeDownloads } = useNotifications();
  const downloadInfo = activeDownloads.find((d) => d.model === model.id);
  const isDownloading = !!downloadInfo;

  const label = modelEntryLabel(model);
  const sizeLabel = formatModelSize(model.sizeBytes);
  const showLocalActions = model.isDownloaded && isLocalModelId(model.id);
  const isDeprecated = model.isDeprecated === true;

  const handleActivate = () => {
    if (model.isDownloaded) {
      onSelect();
      return;
    }
    if (isDownloading) {
      return;
    }
    if (isCloud) {
      onStartTrial();
    } else {
      onDownload();
    }
  };

  return (
    <ModelRowShell
      selected={selected}
      selectable={model.isDownloaded}
      onActivate={handleActivate}
    >
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <LocalModelLabel
            model={model.id}
            label={label}
            className={cn([
              "min-w-0 text-[13px] font-medium",
              (!model.isDownloaded || isDeprecated) && "text-muted-foreground",
            ])}
          />
          {model.engine ? (
            <span className="text-muted-foreground shrink-0 font-mono text-[11px]">
              {model.engine}
            </span>
          ) : null}
          <SttLanguageBadge
            languages={model.languages}
            languageCount={model.languageCount}
          />
          <SttTierBadge tier={model.tier} />
          <LocalModelBackendBadge model={model.id} />
          <ModelModeBadge mode={model.mode} />
        </div>
        <SttModelUseHint
          use={model.recommendedUse}
          pairing={pairing}
          className="mt-0.5 block"
        />
      </div>
      <div className="flex shrink-0 items-center gap-1 self-center">
        {showLocalActions && (
          <LocalModelRowActions model={model.id as LocalModel} />
        )}
        {!model.isDownloaded &&
          (isDownloading ? (
            <span
              className={cn([
                "flex items-center gap-1 rounded-full px-2 py-0.5",
                "border-primary/25 bg-primary/10 text-primary border text-[11px] font-medium",
              ])}
            >
              <Loader2 className="size-3 animate-spin" />
              <span className="font-mono">
                {Math.round(downloadInfo.progress)}%
              </span>
            </span>
          ) : (
            <>
              {sizeLabel && (
                <span className="text-muted-foreground font-mono text-[11px]">
                  {sizeLabel}
                </span>
              )}
              <button
                type="button"
                className={cn([
                  "flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium",
                  "transition-colors duration-(--motion-duration-state)",
                  isCloud
                    ? "border-primary/25 bg-primary text-primary-foreground hover:bg-primary/90"
                    : "border-border text-foreground hover:border-primary/40 hover:text-primary",
                ])}
                onClick={(event) => {
                  event.stopPropagation();
                  handleActivate();
                }}
              >
                {isCloud ? (
                  <Trans>Upgrade to use</Trans>
                ) : (
                  <>
                    <DownloadIcon className="size-3" />
                    <Trans>Download</Trans>
                  </>
                )}
              </button>
            </>
          ))}
      </div>
    </ModelRowShell>
  );
}

function ModelModeBadge({ mode }: { mode?: ModelEntry["mode"] }) {
  if (!mode) {
    return null;
  }

  const isRealtime = mode === "realtime";

  return (
    <Tooltip delayDuration={100}>
      <TooltipTrigger asChild>
        <span
          className={cn([
            "shrink-0 cursor-help rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
            isRealtime ? modelBadgeAccentClassName : modelBadgeNeutralClassName,
          ])}
        >
          {isRealtime ? <Trans>Live</Trans> : <Trans>After recording</Trans>}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64 text-xs">
        {isRealtime ? (
          <Trans>Can transcribe while the meeting is happening.</Trans>
        ) : (
          <Trans>
            Runs after the recording finishes, not during the meeting.
          </Trans>
        )}
      </TooltipContent>
    </Tooltip>
  );
}

function isLocalModelId(model: string): model is LocalModel {
  return isSupportedLocalSttModel(model);
}

function LocalModelRowActions({ model }: { model: LocalModel }) {
  const { t } = useLingui();
  const queryClient = useQueryClient();

  const stopActivate = (event: React.SyntheticEvent<HTMLButtonElement>) => {
    event.stopPropagation();
  };

  const handleOpen = () => {
    const resultPromise = String(model).startsWith("soniqo-")
      ? localSttCommands.soniqoModelDir(model)
      : localSttCommands.modelsDir();

    void resultPromise.then((result) => {
      if (result.status === "ok") {
        void openerCommands.openPath(result.data, null);
      }
    });
  };

  const handleDelete = () => {
    void localSttCommands.deleteModel(model).then((result) => {
      if (result.status === "ok") {
        void queryClient.invalidateQueries({
          queryKey: sttModelQueries.isDownloaded(model).queryKey,
        });
      }
    });
  };

  return (
    <div
      className={cn([
        "flex items-center gap-1",
        "opacity-0 transition-opacity duration-(--motion-duration-state)",
        "group-hover/model-row:opacity-100",
        "group-focus-within/model-row:opacity-100",
      ])}
    >
      <button
        type="button"
        aria-label={t`Show in file manager`}
        className={cn([
          "flex size-6 items-center justify-center rounded-full",
          "text-muted-foreground hover:text-foreground",
        ])}
        onClick={(event) => {
          stopActivate(event);
          handleOpen();
        }}
      >
        <FolderOpen className="size-3.5" />
      </button>
      <button
        type="button"
        aria-label={t`Delete model`}
        className={cn([
          "flex size-6 items-center justify-center rounded-full",
          "text-destructive hover:text-destructive/80",
        ])}
        onClick={(event) => {
          stopActivate(event);
          handleDelete();
        }}
      >
        <Trash2 className="size-3.5" />
      </button>
    </div>
  );
}
