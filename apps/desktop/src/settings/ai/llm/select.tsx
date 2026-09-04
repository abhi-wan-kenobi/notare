import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Channel } from "@tauri-apps/api/core";
import { Check, DownloadIcon, Loader2, Trash2, XIcon } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

import {
  commands as localLlmCommands,
  type GgufLlmModel,
} from "@hypr/plugin-local-llm";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { cn } from "@hypr/utils";

import { useLlmSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { getPreferredProviderModel } from "./selection";
import { type Provider, PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { providerRowId, ProviderIconSlot } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { listAnthropicModels } from "~/settings/ai/shared/list-anthropic";
import { listAzureAIModels } from "~/settings/ai/shared/list-azure-ai";
import { listAzureOpenAIModels } from "~/settings/ai/shared/list-azure-openai";
import { listCloudflareWorkersAIModels } from "~/settings/ai/shared/list-cloudflare-workers-ai";
import {
  type InputModality,
  type ListModelsResult,
} from "~/settings/ai/shared/list-common";
import { listGoogleModels } from "~/settings/ai/shared/list-google";
import { listLMStudioModels } from "~/settings/ai/shared/list-lmstudio";
import { listMistralModels } from "~/settings/ai/shared/list-mistral";
import { listOllamaModels } from "~/settings/ai/shared/list-ollama";
import {
  listGenericModels,
  listOpenAIModels,
} from "~/settings/ai/shared/list-openai";
import { listOpenRouterModels } from "~/settings/ai/shared/list-openrouter";
import { ModelCombobox } from "~/settings/ai/shared/model-combobox";
import { useAiProviders } from "~/settings/providers";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { SettingsAlert } from "~/shared/ui/settings-alert";

export function SelectProviderAndModel() {
  const { t } = useLingui();
  const configuredProviders = useConfiguredMapping();
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const { setAccordionValue } = useLlmSettings();

  const { current_llm_model, current_llm_provider } = useConfigValues([
    "current_llm_model",
    "current_llm_provider",
  ] as const);
  const selectedProviderConfigured = current_llm_provider
    ? (configuredProviders[current_llm_provider]?.configured ?? false)
    : false;

  const health = useConnectionHealth();
  const isConfigured = !!(
    current_llm_provider &&
    current_llm_model &&
    selectedProviderConfigured
  );
  const hasError = isConfigured && health.status === "error";

  const handleSelectProvider = useSetSettingValue("current_llm_provider");
  const handleSelectModel = useSetSettingValue("current_llm_model");
  const lastSelectedModelsRef = useRef<Record<string, string>>(
    current_llm_provider && current_llm_model
      ? { [current_llm_provider]: current_llm_model }
      : {},
  );
  const selectionRequestRef = useRef(0);

  const rememberModel = (provider?: string, model?: string) => {
    if (!provider || model === undefined) {
      return;
    }

    lastSelectedModelsRef.current[provider] = model;
  };

  const getCachedModels = (provider: string) => {
    const status = configuredProviders[provider];
    if (!status?.listModels) {
      return [];
    }

    return (
      queryClient.getQueryData<ListModelsResult>([
        "models",
        provider,
        status.listModels,
      ])?.models ?? []
    );
  };

  const fetchModels = async (provider: string) => {
    const status = configuredProviders[provider];
    const listModels = status?.listModels;
    if (!listModels) {
      return [];
    }

    const result = await queryClient.fetchQuery({
      queryKey: ["models", provider, listModels],
      queryFn: async () => await listModels(),
      retry: 3,
      retryDelay: 300,
      staleTime: 1000 * 2,
    });

    return result.models;
  };

  const handleProviderChange = (provider: string) => {
    if (provider === "hyprnote" && !billing.isPaid) {
      billing.upgradeToPro();
      return;
    }

    const status = configuredProviders[provider];
    if (!status?.listModels) {
      setAccordionValue(provider);
    }

    rememberModel(current_llm_provider, current_llm_model);

    const nextModel = getPreferredProviderModel(
      lastSelectedModelsRef.current[provider],
      getCachedModels(provider),
      { allowSavedModelWithoutChoices: provider === "custom" },
    );

    rememberModel(provider, nextModel);
    handleSelectProvider(provider);
    handleSelectModel(nextModel);

    const requestId = ++selectionRequestRef.current;
    void (async () => {
      const models = await fetchModels(provider);
      const resolvedModel = getPreferredProviderModel(
        lastSelectedModelsRef.current[provider],
        models,
        { allowSavedModelWithoutChoices: provider === "custom" },
      );

      if (selectionRequestRef.current !== requestId) {
        return;
      }

      rememberModel(provider, resolvedModel);
      handleSelectModel(resolvedModel);
    })();
  };

  const handleModelChange = (model: string) => {
    if (!current_llm_provider) {
      return;
    }

    rememberModel(current_llm_provider, model);
    handleSelectModel(model);
  };

  return (
    <div className="flex flex-col gap-4">
      {!isConfigured && (
        <SettingsAlert>
          <Trans>
            <strong className="font-medium">Language model</strong> is needed to
            make Notare summarize and chat about your conversations.
          </Trans>
        </SettingsAlert>
      )}

      {hasError && health.message && (
        <SettingsAlert>{health.message}</SettingsAlert>
      )}

      <h3 className="text-md font-sans font-semibold">
        <Trans>Model being used</Trans>
      </h3>
      <div className="flex flex-row items-center gap-4">
        <div className="min-w-0 flex-2" data-llm-provider-selector>
          <Select
            value={current_llm_provider || ""}
            onValueChange={handleProviderChange}
          >
            <SelectTrigger className="bg-card shadow-none focus:ring-0">
              <SelectValue placeholder={t`Select a provider`} />
            </SelectTrigger>
            <SelectContent>
              {PROVIDERS.map((provider) => {
                const requiresPro = requiresEntitlement(
                  provider.requirements,
                  "pro",
                );
                const locked = requiresPro && !billing.isPaid;
                const configured =
                  configuredProviders[provider.id]?.configured ?? false;

                return (
                  <SelectItem
                    key={provider.id}
                    value={provider.id}
                    disabled={locked || !configured}
                    className={cn([
                      "data-disabled:text-muted-foreground data-disabled:!opacity-100",
                      !configured && !locked && "text-muted-foreground",
                    ])}
                  >
                    <div className="flex flex-col gap-0.5">
                      <div className="flex items-center gap-2">
                        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                        <span>{provider.displayName}</span>
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

        <span className="text-muted-foreground">/</span>

        <div className="min-w-0 flex-3">
          <ModelCombobox
            providerId={current_llm_provider || ""}
            value={current_llm_model || ""}
            onChange={handleModelChange}
            disabled={!current_llm_provider || !selectedProviderConfigured}
            listModels={
              current_llm_provider
                ? configuredProviders[current_llm_provider]?.listModels
                : undefined
            }
            isConfigured={isConfigured && health.status === "success"}
            suffix={isConfigured ? <HealthStatusIndicator /> : undefined}
          />
        </div>
      </div>

      {current_llm_provider === "notare-local" && (
        <LocalModelManager
          onModelDownloaded={() => {
            queryClient.invalidateQueries({
              queryKey: ["models", "notare-local"],
            });
          }}
        />
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000_000) {
    return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  }
  return `${(bytes / 1_000_000).toFixed(0)} MB`;
}

function LocalModelManager({
  onModelDownloaded,
}: {
  onModelDownloaded: () => void;
}) {
  const queryClient = useQueryClient();

  const supportedQuery = useQuery({
    queryKey: ["local-llm", "supported-models"],
    queryFn: async () => {
      const result = await localLlmCommands.listSupportedModel();
      if (result.status !== "ok") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const downloadedQuery = useQuery({
    queryKey: ["local-llm", "downloaded-models"],
    queryFn: async () => {
      const result = await localLlmCommands.listDownloadedModel();
      if (result.status !== "ok") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const downloadedSet = useMemo(
    () => new Set<string>(downloadedQuery.data ?? []),
    [downloadedQuery.data],
  );

  const [downloading, setDownloading] = useState<Map<GgufLlmModel, number>>(
    new Map(),
  );

  const handleDownload = useCallback(
    async (model: GgufLlmModel) => {
      setDownloading((prev) => new Map(prev).set(model, 0));
      try {
        const channel = new Channel<number>();
        channel.onmessage = (progress) => {
          setDownloading((prev) => new Map(prev).set(model, progress));
        };
        const result = await localLlmCommands.downloadModel(model, channel);
        if (result.status !== "ok") {
          throw new Error(result.error);
        }
        queryClient.invalidateQueries({
          queryKey: ["local-llm", "downloaded-models"],
        });
        onModelDownloaded();
      } finally {
        setDownloading((prev) => {
          const next = new Map(prev);
          next.delete(model);
          return next;
        });
      }
    },
    [queryClient, onModelDownloaded],
  );

  const handleCancel = useCallback(async (model: GgufLlmModel) => {
    await localLlmCommands.cancelDownload(model);
    setDownloading((prev) => {
      const next = new Map(prev);
      next.delete(model);
      return next;
    });
  }, []);

  const handleDelete = useCallback(
    async (model: GgufLlmModel) => {
      await localLlmCommands.deleteModel(model);
      queryClient.invalidateQueries({
        queryKey: ["local-llm", "downloaded-models"],
      });
      onModelDownloaded();
    },
    [queryClient, onModelDownloaded],
  );

  if (supportedQuery.isLoading) {
    return (
      <div className="flex items-center gap-2 py-2">
        <Loader2 className="text-muted-foreground size-4 animate-spin" />
        <span className="text-muted-foreground text-sm">
          <Trans>Loading available models…</Trans>
        </span>
      </div>
    );
  }

  const models = supportedQuery.data ?? [];

  return (
    <div className="flex flex-col gap-2">
      <h4 className="text-sm font-medium">
        <Trans>Local models</Trans>
      </h4>
      <p className="text-muted-foreground -mt-1 text-xs">
        <Trans>
          Download a model to run the LLM entirely on your device. No API key
          needed.
        </Trans>
      </p>
      <div className="divide-border/60 divide-y">
        {models.map((model) => {
          const isDownloaded = downloadedSet.has(model.key);
          const progress = downloading.get(model.key);
          const isDownloading = progress !== undefined;

          return (
            <div key={model.key} className="flex items-center gap-3 py-2.5">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium">{model.name}</span>
                  <span className="text-muted-foreground text-xs">
                    {formatBytes(model.size_bytes)}
                  </span>
                  {isDownloaded && (
                    <Check className="text-ok size-3.5 shrink-0" />
                  )}
                </div>
                <p className="text-muted-foreground text-xs">
                  {model.description}
                </p>
                {isDownloading && (
                  <div className="mt-1 flex items-center gap-2">
                    <div className="bg-muted h-1.5 flex-1 overflow-hidden rounded-full">
                      <div
                        className="bg-primary h-full rounded-full transition-all duration-300"
                        style={{ width: `${Math.min(progress, 100)}%` }}
                      />
                    </div>
                    <span className="text-muted-foreground w-9 text-right font-mono text-xs">
                      {Math.round(progress)}%
                    </span>
                  </div>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-1">
                {isDownloading ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleCancel(model.key)}
                  >
                    <XIcon className="size-3.5" />
                    <Trans>Cancel</Trans>
                  </Button>
                ) : isDownloaded ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(model.key)}
                  >
                    <Trash2 className="size-3.5" />
                    <Trans>Delete</Trans>
                  </Button>
                ) : (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleDownload(model.key)}
                  >
                    <DownloadIcon className="size-3.5" />
                    <Trans>Download</Trans>
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export type ProviderStatus = {
  configured: boolean;
  listModels?: () => Promise<ListModelsResult>;
};

type ProviderConfig = {
  base_url?: unknown;
  api_key?: unknown;
};

export function getLlmProviderStatus({
  provider,
  config,
  isAuthenticated,
  isPaid,
}: {
  provider: Provider;
  config?: ProviderConfig;
  isAuthenticated: boolean;
  isPaid: boolean;
}): ProviderStatus {
  const baseUrl = String(config?.base_url || provider.baseUrl || "").trim();
  const apiKey = String(config?.api_key || "").trim();

  const eligible =
    getProviderSelectionBlockers(provider.requirements, {
      isAuthenticated,
      isPaid,
      config: { base_url: baseUrl, api_key: apiKey },
    }).length === 0;

  if (!eligible) {
    return { configured: false };
  }

  if (provider.id === "hyprnote") {
    const result: ListModelsResult = {
      models: ["Auto"],
      ignored: [],
      metadata: {
        Auto: {
          input_modalities: ["text", "image"] as InputModality[],
        },
      },
    };
    return { configured: true, listModels: async () => result };
  }

  if (provider.id === "notare-local") {
    return {
      configured: true,
      listModels: async () => {
        const result = await localLlmCommands.listDownloadedModel();
        const models = result.status === "ok" ? result.data.map((m) => m) : [];
        return { models, ignored: [], metadata: {} };
      },
    };
  }

  let listModelsFunc: () => Promise<ListModelsResult>;

  switch (provider.id) {
    case "openai":
      listModelsFunc = () => listOpenAIModels(baseUrl, apiKey);
      break;
    case "cloudflare_workers_ai":
      listModelsFunc = () => listCloudflareWorkersAIModels(baseUrl, apiKey);
      break;
    case "anthropic":
      listModelsFunc = () => listAnthropicModels(baseUrl, apiKey);
      break;
    case "openrouter":
      listModelsFunc = () => listOpenRouterModels(baseUrl, apiKey);
      break;
    case "google_generative_ai":
      listModelsFunc = () => listGoogleModels(baseUrl, apiKey);
      break;
    case "mistral":
      listModelsFunc = () => listMistralModels(baseUrl, apiKey);
      break;
    case "azure_openai":
      listModelsFunc = () => listAzureOpenAIModels(baseUrl, apiKey);
      break;
    case "azure_ai":
      listModelsFunc = () => listAzureAIModels(baseUrl, apiKey);
      break;
    case "ollama":
      listModelsFunc = () => listOllamaModels(baseUrl, apiKey);
      break;
    case "lmstudio":
      listModelsFunc = () => listLMStudioModels(baseUrl, apiKey);
      break;
    case "custom":
      listModelsFunc = () => listGenericModels(baseUrl, apiKey);
      break;
    default:
      listModelsFunc = () => listGenericModels(baseUrl, apiKey);
  }

  return { configured: true, listModels: listModelsFunc };
}

// Exported for reuse by `./scoped-models` (the per-task model overrides
// section): both need the exact same "which providers are actually usable
// right now" mapping, so this is the single source of truth for it rather
// than a second copy of the eligibility logic.
export function useConfiguredMapping(): Record<string, ProviderStatus> {
  const auth = useAuth();
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders("llm");

  const mapping = useMemo(() => {
    return Object.fromEntries(
      PROVIDERS.map((provider) => {
        const config = configuredProviders[providerRowId("llm", provider.id)];
        return [
          provider.id,
          getLlmProviderStatus({
            provider,
            config,
            isAuthenticated: !!auth?.session,
            isPaid: billing.isPaid,
          }),
        ];
      }),
    ) as Record<string, ProviderStatus>;
  }, [configuredProviders, auth, billing]);

  return mapping;
}
