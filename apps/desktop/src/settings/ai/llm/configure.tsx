import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";

import { useLlmSettings } from "./context";
import { OpenRouterSignIn } from "./openrouter-sign-in";
import {
  ProviderId,
  PROVIDERS,
  STANDARD_PROVIDERS,
  SUBSCRIPTION_PROVIDERS,
  type SubscriptionProviderId,
  toLlmAuthProvider,
} from "./shared";
import { SubscriptionCard } from "./subscription-card";

import {
  commands as llmAuthCommands,
  DEFAULT_CATALOG,
  type LlmAuthCatalogProvider,
} from "~/ai/providers/llm-auth-client";
import {
  OMP_DEFAULT_BASE_URL,
  probeOmpGateway,
} from "~/services/llm-router/local-discovery";
import { NonHyprProviderCard, StyledStreamdown } from "~/settings/ai/shared";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useLlmSettings();

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-sans font-semibold">
        <Trans>Configure Providers</Trans>
      </h3>
      <Accordion
        type="single"
        collapsible
        className="flex flex-col gap-3"
        value={accordionValue}
        onValueChange={setAccordionValue}
      >
        {STANDARD_PROVIDERS.map((provider) => (
          <NonHyprProviderCard
            key={provider.id}
            config={provider}
            providerType="llm"
            providers={PROVIDERS}
            providerContext={<ProviderContext providerId={provider.id} />}
            testConnection={
              provider.id === "omp"
                ? (values) => (
                    <OmpDetectHint
                      baseUrl={values.base_url}
                      apiKey={values.api_key}
                    />
                  )
                : undefined
            }
            signIn={
              provider.id === "openrouter"
                ? ({ setApiKey }) => <OpenRouterSignIn setApiKey={setApiKey} />
                : undefined
            }
          />
        ))}
      </Accordion>
      <SubscriptionProvidersSection />
    </div>
  );
}
export function SubscriptionProvidersSection() {
  const catalogQuery = useQuery({
    queryKey: ["llm-auth", "catalog"],
    queryFn: async () => {
      const res = await llmAuthCommands.catalog();
      return res.status === "ok" ? res.data : DEFAULT_CATALOG;
    },
    staleTime: 1000 * 60 * 5,
  });

  const catalog = catalogQuery.data ?? DEFAULT_CATALOG;
  const enabledProviders = SUBSCRIPTION_PROVIDERS.map((provider) => {
    const pluginId = toLlmAuthProvider(provider.id as SubscriptionProviderId);
    const catalogProvider = catalog.providers.find((cp) => cp.id === pluginId);
    return { provider, catalogProvider };
  }).filter(
    (
      item,
    ): item is {
      provider: (typeof SUBSCRIPTION_PROVIDERS)[number];
      catalogProvider: LlmAuthCatalogProvider;
    } => Boolean(item.catalogProvider?.enabled),
  );

  if (enabledProviders.length === 0) {
    return null;
  }

  return (
    <Accordion type="single" collapsible className="flex flex-col gap-3">
      <AccordionItem
        value="subscription-section"
        className="border-border/60 bg-card rounded-xl border shadow-xs"
      >
        <AccordionTrigger className="px-4 py-3 hover:no-underline">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold">
              <Trans>Use my subscription (unofficial)</Trans>
            </span>
            <Badge
              variant="outline"
              className="border-amber-500/30 bg-amber-500/10 text-[10px] text-amber-600 dark:text-amber-400"
            >
              <Trans>Unofficial</Trans>
            </Badge>
          </div>
        </AccordionTrigger>
        <AccordionContent className="flex flex-col gap-3 px-4 pt-1 pb-4">
          <p className="text-muted-foreground text-xs leading-relaxed">
            <Trans>
              Sign in with your Claude, ChatGPT, or Google account to use your
              subscription directly without entering API keys.
            </Trans>
          </p>
          <div className="flex flex-col gap-3">
            {enabledProviders.map(({ provider, catalogProvider }) => (
              <SubscriptionCard
                key={provider.id}
                provider={provider}
                catalogProvider={catalogProvider}
              />
            ))}
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "lmstudio"
      ? "- Ensure LM Studio server is **running.** (Default port is 1234)\n- Enable **CORS** in LM Studio config."
      : providerId === "ollama"
        ? "- Ensure Ollama is **running** (`ollama serve`)\n- Pull a model first (`ollama pull llama3.2`)"
        : providerId === "omp"
          ? "- Run `omp auth-gateway token` to print your bearer token.\n- API key is optional if the gateway was started with `--no-auth`."
          : providerId === "custom"
            ? "We only support **OpenAI-compatible** endpoints for now."
            : providerId === "openrouter"
              ? "We filter out models from the combobox based on heuristics like **input modalities** and **tool support**."
              : providerId === "azure_openai"
                ? "Enter your **Azure OpenAI endpoint** (e.g. `https://your-resource.openai.azure.com`) as the Base URL and your **API key**. [Report issues](https://github.com/fastrepl/char/issues/3928)"
                : providerId === "azure_ai"
                  ? "Enter your **Azure AI Foundry endpoint** as the Base URL and your **API key**. Supports Claude and other models deployed via Azure AI Foundry. [Report issues](https://github.com/fastrepl/char/issues/3928)"
                  : providerId === "google_generative_ai"
                    ? "Visit [AI Studio](https://aistudio.google.com/api-keys) to create an API key."
                    : providerId === "cloudflare_workers_ai"
                      ? "Enter the Workers AI **OpenAI-compatible base URL** as `https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1` and use a Cloudflare API token with Workers AI access."
                      : "";

  if (!content) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content}</StyledStreamdown>;
}

function OmpDetectHint({
  baseUrl,
  apiKey,
}: {
  baseUrl: string;
  apiKey: string;
}) {
  const { t } = useLingui();
  const effectiveBaseUrl = baseUrl?.trim() || OMP_DEFAULT_BASE_URL;

  const probeQuery = useQuery({
    queryKey: ["omp-gateway-probe", effectiveBaseUrl, apiKey],
    queryFn: async () => probeOmpGateway(effectiveBaseUrl, apiKey),
    retry: false,
    staleTime: 5000,
  });

  const handleDetect = () => {
    void probeQuery.refetch();
  };

  const status = probeQuery.data?.status;
  const count = probeQuery.data?.models.length ?? 0;

  return (
    <div className="border-border/60 bg-muted/40 flex flex-col gap-2 rounded-lg border p-3 text-xs">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          {probeQuery.isFetching ? (
            <span className="bg-muted-foreground flex h-2 w-2 animate-pulse rounded-full" />
          ) : status === "available" ? (
            <span className="flex h-2 w-2 rounded-full bg-emerald-500" />
          ) : status === "needs_token" ? (
            <span className="flex h-2 w-2 rounded-full bg-amber-500" />
          ) : (
            <span className="bg-muted-foreground/50 flex h-2 w-2 rounded-full" />
          )}
          <span className="text-foreground font-medium">
            {probeQuery.isFetching
              ? t`Probing omp gateway…`
              : status === "available"
                ? count > 0
                  ? t`omp gateway detected (${count} models)`
                  : t`omp gateway detected`
                : status === "needs_token"
                  ? t`omp gateway detected (bearer token required)`
                  : t`omp gateway not detected`}
          </span>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 text-xs"
          onClick={handleDetect}
          disabled={probeQuery.isFetching}
        >
          {probeQuery.isFetching ? t`Detecting…` : t`Detect`}
        </Button>
      </div>
      {status === "needs_token" && (
        <p className="text-muted-foreground">
          <Trans>
            Gateway found, but requires authentication. Run{" "}
            <code className="bg-background/80 rounded px-1 py-0.5 font-mono">
              omp auth-gateway token
            </code>{" "}
            to print your bearer token.
          </Trans>
        </p>
      )}
      {status === "unavailable" && (
        <p className="text-muted-foreground">
          <Trans>
            Start the gateway with{" "}
            <code className="bg-background/80 rounded px-1 py-0.5 font-mono">
              omp auth-gateway serve --bind=127.0.0.1:4000
            </code>
          </Trans>
        </p>
      )}
    </div>
  );
}
