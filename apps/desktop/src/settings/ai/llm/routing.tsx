import { Trans } from "@lingui/react/macro";
import { useId, type ReactNode } from "react";

import { Switch } from "@hypr/ui/components/ui/switch";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

/**
 * LLM routing preferences (WS-A). Surfaces the router invariants the user can
 * actually control or should know about:
 *
 * - the capability-override toggle (bound to the existing `llm_caps_override`
 *   setting), which lets a smaller / unknown model be used for structured
 *   tasks like action items;
 * - a read-only explainer of the local-first routing rule (cloud providers are
 *   only used when explicitly selected here) — no new persisted key, this just
 *   describes existing behavior.
 */
export function RoutingPreferences() {
  const capsOverride = useConfigValue("llm_caps_override");
  const setCapsOverride = useSetSettingValue("llm_caps_override");

  return (
    <section className="flex flex-col gap-4">
      <h3 className="text-md font-sans font-semibold">
        <Trans>Routing</Trans>
      </h3>

      <RoutingRow
        title={<Trans>Allow smaller models for structured tasks</Trans>}
        description={
          <Trans>
            Use the selected model for action items and other structured-output
            tasks even when its size can't be confirmed. Turn this on only if
            you trust your model's output quality; Notare still rejects
            malformed results downstream.
          </Trans>
        }
        checked={capsOverride === true}
        onChange={setCapsOverride}
      />

      <p className="text-muted-foreground text-xs">
        <Trans>
          Local-first: Notare prefers a local model (Ollama or LM Studio) and
          only uses a cloud provider when you explicitly select one above.
        </Trans>
      </p>
    </section>
  );
}

function RoutingRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: ReactNode;
  description: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const titleId = useId();
  const descriptionId = useId();

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          {title}
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          {description}
        </p>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onChange}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      />
    </div>
  );
}
