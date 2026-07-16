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
} from "lucide-react";
import { useRef } from "react";

import {
  commands as localSttCommands,
  type LocalModel,
  type SttModelLanguages,
  type SttModelTier,
  type SttRecommendedUse,
} from "@hypr/plugin-local-stt";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import type { AIProviderStorage } from "@hypr/store";
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
  isRealtimeLocalModel,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
  isSupportedLocalSttModel,
} from "~/stt/capabilities";
import {
  getDefaultSttModel,
  getPreferredProviderModel,
} from "~/stt/model-selection";

export function SelectProviderAndModel() {
  const { t } = useLingui();
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const billing = useBillingAccess();
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();

  const selectedSttModel = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : undefined;
  const isConfigured = !!(current_stt_provider && selectedSttModel);
  const hasError = isConfigured && health.status === "error";
  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const selectedModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];
  const displayedSttModel =
    selectedProvider === "custom"
      ? selectedSttModel
      : getPreferredProviderModel(selectedSttModel, selectedModels, {
          keepUnavailableSavedModel: true,
        });
  const handleSelectProvider = useSetSettingValue("current_stt_provider");

  const handleSelectModel = useSetSettingValue("current_stt_model");
  const handleSelectFinalModel = useSetSettingValue("final_stt_model");
  const lastSelectedModelsRef = useRef<Record<string, string>>(
    current_stt_provider && selectedSttModel
      ? { [current_stt_provider]: selectedSttModel }
      : {},
  );
  const rememberModel = (provider?: string, model?: string) => {
    if (!provider || model === undefined) {
      return;
    }

    lastSelectedModelsRef.current[provider] = model;
  };

  const handleProviderChange = (provider: string) => {
    rememberModel(current_stt_provider, selectedSttModel);

    const providerId = provider as ProviderId;
    const nextModels = configuredProviders[providerId]?.models ?? [];
    const nextModel =
      getPreferredProviderModel(
        lastSelectedModelsRef.current[provider],
        nextModels,
        { allowSavedModelWithoutChoices: providerId === "custom" },
      ) ||
      getDefaultSttModel(providerId) ||
      "";

    rememberModel(provider, nextModel);
    handleSelectProvider(provider);
    handleSelectModel(nextModel);
    // The final model belongs to the previous provider's catalog; reset it to
    // "same as live" so a stale id never leaks across providers.
    handleSelectFinalModel("");
  };

  const handleModelChange = (model: string) => {
    if (!current_stt_provider) {
      return;
    }

    rememberModel(current_stt_provider, model);
    handleSelectModel(model);
  };
  return (
    <div className="flex flex-col gap-4">
      {!isConfigured && (
        <SettingsAlert>
          <Trans>
            <strong className="font-medium">Transcription model</strong> is
            needed to make Notare listen to your conversations.
          </Trans>
        </SettingsAlert>
      )}

      {hasError && health.message && (
        <SettingsAlert>{health.message}</SettingsAlert>
      )}

      <div className="flex items-center gap-2">
        <h3 className="text-md font-sans font-semibold">
          <Trans>Live transcription</Trans>
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
          Transcribes while the meeting is happening. Pick a provider, then the
          model that listens live.
        </Trans>
      </p>

      <div className="max-w-md" data-stt-provider-selector>
        <Select
          value={current_stt_provider || ""}
          onValueChange={handleProviderChange}
        >
          <SelectTrigger className="bg-card shadow-none focus:ring-0">
            <SelectValue placeholder={t`Select a provider`} />
          </SelectTrigger>
          <SelectContent>
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

      {current_stt_provider === "custom" ? (
        <Input
          value={displayedSttModel || ""}
          onChange={(event) => handleModelChange(event.target.value)}
          className="text-xs"
          placeholder={t`Enter a model identifier`}
        />
      ) : current_stt_provider ? (
        <ModelRowList
          testId="stt-live-model-list"
          models={selectedModels}
          selectedId={displayedSttModel}
          pairing="live"
          onSelect={handleModelChange}
          onDownload={(model) => startDownload(model as LocalModel)}
          onStartTrial={startTrial}
        />
      ) : null}
    </div>
  );
}

// Sentinel Select value meaning "no dedicated final model" (Radix Select
// items cannot use an empty-string value). Stored as "" in settings.
const FINAL_MODEL_SAME_AS_LIVE = "__same_as_live__";

export function SelectFinalModel() {
  const { t } = useLingui();
  const { current_stt_provider, final_stt_model } = useConfigValues([
    "current_stt_provider",
    "final_stt_model",
  ] as const);
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const handleSelectFinalModel = useSetSettingValue("final_stt_model");

  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const providerModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];

  if (!selectedProvider) {
    return null;
  }

  const finalModel = final_stt_model?.trim() ?? "";
  const selectedFinalModel = providerModels.find(
    (model) => model.id === finalModel,
  );

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-md font-sans font-semibold">
        <Trans>Final pass & re-transcription</Trans>
      </h3>
      <p className="text-muted-foreground text-sm">
        <Trans>
          Used for the accurate pass after a recording ends and when
          re-transcribing a note. Pick a larger model than the live one for
          better quality, or keep it the same as the live model.
        </Trans>
      </p>
      {selectedProvider === "custom" ? (
        <Input
          value={finalModel}
          onChange={(event) => handleSelectFinalModel(event.target.value)}
          className="text-xs"
          placeholder={t`Same as live model`}
        />
      ) : (
        <ModelRowList
          testId="stt-final-model-list"
          models={providerModels}
          selectedId={
            selectedFinalModel
              ? selectedFinalModel.id
              : FINAL_MODEL_SAME_AS_LIVE
          }
          pairing="final"
          onSelect={(id) =>
            handleSelectFinalModel(id === FINAL_MODEL_SAME_AS_LIVE ? "" : id)
          }
          onDownload={(model) => startDownload(model as LocalModel)}
          onStartTrial={startTrial}
          leadingRow={{
            id: FINAL_MODEL_SAME_AS_LIVE,
            label: <Trans>Same as live model</Trans>,
            hint: <Trans>Reuses the live model for the accurate pass.</Trans>,
          }}
        />
      )}
    </div>
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
  // whisper.cpp models run everywhere; soniqo/argmax backends are Apple
  // Silicon only.
  const visibleLocalModels = localModels.filter(
    (m) => m.model_type === "whispercpp" || isAppleSilicon,
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
        return [provider.id, { configured: false, models: [] }];
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

        return [provider.id, { configured: true, models }];
      }

      if (provider.id === "custom") {
        return [provider.id, { configured: true, models: [] }];
      }

      return [
        provider.id,
        {
          configured: true,
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
