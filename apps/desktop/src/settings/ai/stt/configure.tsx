import { Trans } from "@lingui/react/macro";

import { Accordion } from "@hypr/ui/components/ui/accordion";

import { useSttSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";
import { TestConnectionButton } from "./test-connection-button";

import { NonHyprProviderCard, StyledStreamdown } from "~/settings/ai/shared";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useSttSettings();

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
        {PROVIDERS.filter((provider) => provider.id !== "hyprnote").map(
          (provider) => (
            <NonHyprProviderCard
              key={provider.id}
              config={provider}
              providerType="stt"
              providers={PROVIDERS}
              providerContext={<ProviderContext providerId={provider.id} />}
              // The "Custom" provider is also how a self-hosted Notare STT
              // companion server (docs/stt-server-design.md issue #14) plugs
              // in — its base_url/api_key flow through the same
              // ListenClient/BatchClient plumbing as every other STT
              // provider (see docs/stt-server-design.md §5). Offer a live
              // connectivity probe against its `/api/status` endpoint.
              testConnection={
                provider.id === "custom"
                  ? (values) => (
                      <TestConnectionButton
                        baseUrl={values.base_url}
                        apiKey={values.api_key}
                      />
                    )
                  : undefined
              }
            />
          ),
        )}
      </Accordion>
    </div>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "hyprnote"
      ? "**Notare Cloud** routes request to the **best available model** for highest accuracy and performance."
      : providerId === "deepgram"
        ? `Use [Deepgram](https://deepgram.com) for transcriptions. \
    If you want to use a [Dedicated](https://developers.deepgram.com/reference/custom-endpoints#deepgram-dedicated-endpoints)
    or [EU](https://developers.deepgram.com/reference/custom-endpoints#eu-endpoints) endpoint,
    you can do that in the **advanced** section.`
        : providerId === "soniox"
          ? `Use [Soniox](https://soniox.com) for transcriptions.`
          : providerId === "assemblyai"
            ? `Use [AssemblyAI](https://www.assemblyai.com) for transcriptions.`
            : providerId === "gladia"
              ? `Use [Gladia](https://www.gladia.io) for transcriptions.`
              : providerId === "openai"
                ? `Use [OpenAI](https://openai.com) for transcriptions.`
                : providerId === "cloudflare_workers_ai"
                  ? `Use a [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/) endpoint that exposes Deepgram-compatible Nova-3 transcription.`
                  : providerId === "fireworks"
                    ? `Use [Fireworks AI](https://fireworks.ai) for transcriptions.`
                    : providerId === "mistral"
                      ? `Use [Mistral](https://mistral.ai) for transcriptions.`
                      : providerId === "custom"
                        ? `We only support **Deepgram compatible** endpoints for now. \
    This is also where a self-hosted [Notare STT companion server](https://github.com/abhi-wan-kenobi/notare) \
    on your LAN plugs in — enter its base URL (e.g. \`http://<host>:8383/v1\`), \
    an API key only if you enabled one on the server, and use **Test connection**
    to confirm it's reachable.`
                        : "";

  if (!content.trim()) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content.trim()}</StyledStreamdown>;
}
