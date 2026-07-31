import { Trans, useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";

import { useConfiguredMapping, type ProviderStatus } from "./select";
import { PROVIDERS } from "./shared";

import { isCloudProvider } from "~/ai/scope";
import { ModelCombobox } from "~/settings/ai/shared/model-combobox";
import { useSetSettingValues } from "~/settings/queries";
import { useAiProvider } from "~/settings/providers";
import { useConfigValues } from "~/shared/config";

/**
 * Sentinel select value for "inherit the model chosen above" - Radix Select
 * treats an empty string as no-selection/placeholder, so the persisted empty
 * string (`ai_scope_*_provider === ""`) is mapped to/from this locally.
 */
const USE_DEFAULT_SENTINEL = "__default__";

/**
 * Per-task model overrides (Lane A2): lets cleanup (dictation), notes
 * (summaries) and chat each pin a different provider/model instead of always
 * using the one model picked in "Model being used" above. Everything
 * defaults to "Use default model" (both `ai_scope_*_provider` and
 * `ai_scope_*_model` empty) - the engine's scope resolution falls back to
 * `current_llm_provider`/`current_llm_model` whenever a scope's override is
 * unset, so leaving this collapsed and untouched is a fully valid state.
 *
 * Collapsed by default (`<details>`) since most people never need this -
 * it's meant to read as an escape hatch, not a decision every user has to
 * make.
 */
export function ScopedModelSettings() {
  const { t } = useLingui();
  const configuredProviders = useConfiguredMapping();
  const setValues = useSetSettingValues();

  const {
    ai_scope_cleanup_provider,
    ai_scope_cleanup_model,
    ai_scope_notes_provider,
    ai_scope_notes_model,
    ai_scope_chat_provider,
    ai_scope_chat_model,
    current_llm_provider,
  } = useConfigValues([
    "ai_scope_cleanup_provider",
    "ai_scope_cleanup_model",
    "ai_scope_notes_provider",
    "ai_scope_notes_model",
    "ai_scope_chat_provider",
    "ai_scope_chat_model",
    "current_llm_provider",
  ] as const);

  // Mirror the engine's per-scope invariant (`~/ai/scope.ts`
  // `resolveScopeSelection`): a scope override may only route to a cloud
  // provider when cloud is already opted into globally, i.e. the model
  // picked in "Model being used" above is itself a cloud provider. Filtering
  // cloud providers out of these pickers until that's true keeps this UI
  // from ever offering a choice the engine would silently reject and fall
  // back from.
  const currentProviderConfig = useAiProvider("llm", current_llm_provider);
  const currentProviderDef = PROVIDERS.find(
    (provider) => provider.id === current_llm_provider,
  );
  const globalIsCloud = isCloudProvider(
    current_llm_provider,
    currentProviderConfig?.base_url || currentProviderDef?.baseUrl,
  );

  return (
    <section>
      <details className="group flex flex-col gap-4">
        <summary className="text-muted-foreground hover:text-foreground cursor-pointer text-sm font-medium select-none hover:underline">
          <Trans>Advanced: per-task models</Trans>
        </summary>
        <div className="mt-4 flex flex-col gap-4">
          <p className="text-muted-foreground text-xs">
            <Trans>
              Override the model used for specific tasks. Anything left on
              "Use default model" falls back to the model selected above.
            </Trans>
          </p>
          {!globalIsCloud ? (
            <p className="text-muted-foreground text-xs">
              <Trans>
                Cloud providers won't appear here until you pick one as the
                default model above - an override can't reach the cloud on
                its own.
              </Trans>
            </p>
          ) : null}
          <div className="flex flex-col gap-3">
            <ScopedModelRow
              label={<Trans>Cleanup (dictation)</Trans>}
              ariaLabel={t`Cleanup (dictation) model`}
              providerId={ai_scope_cleanup_provider ?? ""}
              modelId={ai_scope_cleanup_model ?? ""}
              configuredProviders={configuredProviders}
              globalIsCloud={globalIsCloud}
              onChangeProvider={(providerId) =>
                setValues({
                  ai_scope_cleanup_provider: providerId,
                  ai_scope_cleanup_model: "",
                })
              }
              onChangeModel={(modelId) =>
                setValues({ ai_scope_cleanup_model: modelId })
              }
              onUseDefault={() =>
                setValues({
                  ai_scope_cleanup_provider: "",
                  ai_scope_cleanup_model: "",
                })
              }
            />
            <ScopedModelRow
              label={<Trans>Notes (summaries)</Trans>}
              ariaLabel={t`Notes (summaries) model`}
              providerId={ai_scope_notes_provider ?? ""}
              modelId={ai_scope_notes_model ?? ""}
              configuredProviders={configuredProviders}
              globalIsCloud={globalIsCloud}
              onChangeProvider={(providerId) =>
                setValues({
                  ai_scope_notes_provider: providerId,
                  ai_scope_notes_model: "",
                })
              }
              onChangeModel={(modelId) =>
                setValues({ ai_scope_notes_model: modelId })
              }
              onUseDefault={() =>
                setValues({
                  ai_scope_notes_provider: "",
                  ai_scope_notes_model: "",
                })
              }
            />
            <ScopedModelRow
              label={<Trans>Chat</Trans>}
              ariaLabel={t`Chat model`}
              providerId={ai_scope_chat_provider ?? ""}
              modelId={ai_scope_chat_model ?? ""}
              configuredProviders={configuredProviders}
              globalIsCloud={globalIsCloud}
              onChangeProvider={(providerId) =>
                setValues({
                  ai_scope_chat_provider: providerId,
                  ai_scope_chat_model: "",
                })
              }
              onChangeModel={(modelId) =>
                setValues({ ai_scope_chat_model: modelId })
              }
              onUseDefault={() =>
                setValues({
                  ai_scope_chat_provider: "",
                  ai_scope_chat_model: "",
                })
              }
            />
          </div>
        </div>
      </details>
    </section>
  );
}

export function ScopedModelRow({
  label,
  ariaLabel,
  providerId,
  modelId,
  configuredProviders,
  globalIsCloud = true,
  onChangeProvider,
  onChangeModel,
  onUseDefault,
}: {
  label: ReactNode;
  /** Plain-string label for the provider select's `aria-label`. */
  ariaLabel: string;
  /** Empty string = "use default model" (inherit `current_llm_provider`). */
  providerId: string;
  modelId: string;
  configuredProviders: Record<string, ProviderStatus>;
  /**
   * Whether the global "Model being used" selection is itself a cloud
   * provider. Defaults to true (no extra filtering) so callers that don't
   * care about the cloud-opt-in gate - e.g. isolated unit tests - keep the
   * old plain "configured providers only" behavior.
   */
  globalIsCloud?: boolean;
  onChangeProvider: (providerId: string) => void;
  onChangeModel: (modelId: string) => void;
  onUseDefault: () => void;
}) {
  const { t } = useLingui();

  // Only providers the user has actually configured/opted into are offered
  // here - unlike the main "Model being used" picker (which lists every
  // provider and disables the unconfigured ones), an override picker with
  // dead options would just be confusing since there's no inline "configure
  // it here" affordance in this compact a row. Cloud providers are further
  // filtered out unless cloud is already opted into globally, mirroring the
  // engine's `resolveScopeSelection` invariant (`~/ai/scope.ts`) so this
  // picker never offers a choice the engine would silently reject.
  const availableProviders = PROVIDERS.filter((provider) => {
    if (!configuredProviders[provider.id]?.configured) return false;
    if (!globalIsCloud && isCloudProvider(provider.id, provider.baseUrl)) {
      return false;
    }
    return true;
  });
  const providerListModels = providerId
    ? configuredProviders[providerId]?.listModels
    : undefined;

  return (
    <div className="flex flex-wrap items-center gap-3">
      <span className="text-muted-foreground w-36 shrink-0 text-xs font-medium">
        {label}
      </span>
      <div className="min-w-0 flex-1">
        <Select
          value={providerId || USE_DEFAULT_SENTINEL}
          onValueChange={(value) => {
            if (value === USE_DEFAULT_SENTINEL) {
              onUseDefault();
            } else {
              onChangeProvider(value);
            }
          }}
        >
          <SelectTrigger
            className="bg-card shadow-none focus:ring-0"
            aria-label={ariaLabel}
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={USE_DEFAULT_SENTINEL}>
              <Trans>Use default model</Trans>
            </SelectItem>
            {availableProviders.map((provider) => (
              <SelectItem key={provider.id} value={provider.id}>
                {provider.displayName}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {providerId ? (
        <>
          <span className="text-muted-foreground" aria-hidden>
            /
          </span>
          <div className="min-w-0 flex-1">
            <ModelCombobox
              providerId={providerId}
              value={modelId}
              onChange={onChangeModel}
              listModels={providerListModels}
              placeholder={t`Select a model`}
            />
          </div>
        </>
      ) : null}
    </div>
  );
}
