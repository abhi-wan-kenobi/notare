import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2Icon, Loader2Icon, XCircleIcon } from "lucide-react";
import { useEffect, useState } from "react";

import {
  commands as webhookCommands,
  type DeliveryRecord,
  type WebhookSettings,
} from "@hypr/plugin-webhook";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";

const SETTINGS_QUERY_KEY = ["settings", "webhook"] as const;
const DELIVERIES_QUERY_KEY = ["settings", "webhook", "deliveries"] as const;

function unwrap<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: string },
): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function SettingsWebhook() {
  const { data, isLoading, error } = useQuery({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: async () => unwrap(await webhookCommands.getSettings()),
  });

  if (error) {
    throw error;
  }

  if (isLoading || !data) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label="Loading webhook settings"
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return <WebhookSettingsContent settings={data} />;
}

function WebhookSettingsContent({ settings }: { settings: WebhookSettings }) {
  const { t } = useLingui();
  const queryClient = useQueryClient();

  const [endpointUrl, setEndpointUrl] = useState(settings.endpoint_url ?? "");
  const [enabled, setEnabled] = useState(settings.enabled ?? false);
  const [actionItems, setActionItems] = useState(
    settings.events?.action_items_updated ?? false,
  );
  const [sessionEnhanced, setSessionEnhanced] = useState(
    settings.events?.session_enhanced ?? false,
  );
  const [secretInput, setSecretInput] = useState("");
  const [hasSecret, setHasSecret] = useState(settings.has_secret ?? false);

  const persist = useMutation({
    mutationFn: async (next: WebhookSettings) => {
      unwrap(await webhookCommands.setSettings(next));
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
    },
  });

  // Persist the config (never the secret) whenever a field changes.
  const save = (overrides: Partial<WebhookSettings> = {}) => {
    persist.mutate({
      endpoint_url: endpointUrl.trim(),
      enabled,
      events: {
        action_items_updated: actionItems,
        session_enhanced: sessionEnhanced,
      },
      has_secret: hasSecret,
      ...overrides,
    });
  };

  const saveSecret = useMutation({
    mutationFn: async (secret: string) => {
      unwrap(await webhookCommands.setSecret(secret));
    },
    onSuccess: () => {
      setHasSecret(true);
      setSecretInput("");
      void queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
    },
  });

  const clearSecret = useMutation({
    mutationFn: async () => {
      unwrap(await webhookCommands.clearSecret());
    },
    onSuccess: () => {
      setHasSecret(false);
      setSecretInput("");
      void queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
    },
  });

  const test = useMutation({
    mutationFn: async () => unwrap(await webhookCommands.testWebhook()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: DELIVERIES_QUERY_KEY });
    },
  });

  const canConfigure = endpointUrl.trim().length > 0;
  const canTest = enabled && canConfigure && hasSecret;

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Webhooks</Trans>} />

      <p className="text-muted-foreground text-xs">
        <Trans>
          Send Notare events to an endpoint you control. Off by default —
          nothing leaves your device unless you enable it and pick which events
          to share. Each request is signed with HMAC-SHA256 in the{" "}
          <code>X-Notare-Signature</code> header so your receiver can verify it.
        </Trans>
      </p>

      {/* Master switch */}
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">
            <Trans>Enable webhooks</Trans>
          </h3>
          <p className="text-muted-foreground text-xs">
            <Trans>Master switch for outbound delivery.</Trans>
          </p>
        </div>
        <Switch
          checked={enabled}
          onCheckedChange={(v) => {
            setEnabled(v);
            save({ enabled: v });
          }}
        />
      </div>

      {/* Endpoint */}
      <div className="flex flex-col gap-2">
        <h3 className="text-sm font-medium">
          <Trans>Endpoint URL</Trans>
        </h3>
        <Input
          type="url"
          placeholder="https://example.com/notare/webhook"
          value={endpointUrl}
          onChange={(e) => setEndpointUrl(e.target.value)}
          onBlur={() => save()}
        />
      </div>

      {/* Secret */}
      <div className="flex flex-col gap-2">
        <h3 className="text-sm font-medium">
          <Trans>Signing secret</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          {hasSecret ? (
            <Trans>
              A secret is stored in your OS keychain. Enter a new value to
              rotate it.
            </Trans>
          ) : (
            <Trans>
              Set a shared secret. Requests are refused (no-op) until one is
              set.
            </Trans>
          )}
        </p>
        <div className="flex items-center gap-2">
          <Input
            type="password"
            placeholder={hasSecret ? "••••••••" : t`Enter a signing secret`}
            value={secretInput}
            onChange={(e) => setSecretInput(e.target.value)}
          />
          <Button
            type="button"
            variant="outline"
            disabled={secretInput.trim().length === 0 || saveSecret.isPending}
            onClick={() => saveSecret.mutate(secretInput.trim())}
          >
            {hasSecret ? <Trans>Rotate</Trans> : <Trans>Save</Trans>}
          </Button>
          {hasSecret && (
            <Button
              type="button"
              variant="ghost"
              disabled={clearSecret.isPending}
              onClick={() => clearSecret.mutate()}
            >
              <Trans>Clear</Trans>
            </Button>
          )}
        </div>
      </div>

      {/* Per-event opt-in */}
      <div className="flex flex-col gap-4">
        <div className="flex items-center gap-4 pt-2 pb-1">
          <div className="border-muted min-w-0 flex-1 border-t" />
          <span className="text-muted-foreground shrink-0 text-xs font-medium">
            <Trans>Events to send</Trans>
          </span>
          <div className="border-muted min-w-0 flex-1 border-t" />
        </div>

        <EventToggle
          title={<Trans>Action items updated</Trans>}
          description={<Trans>Fires when a note's action items change.</Trans>}
          eventType="action_items.updated"
          checked={actionItems}
          disabled={!enabled}
          onChange={(v) => {
            setActionItems(v);
            save({
              events: {
                action_items_updated: v,
                session_enhanced: sessionEnhanced,
              },
            });
          }}
        />

        <EventToggle
          title={<Trans>Session enhanced</Trans>}
          description={
            <Trans>Fires when a session's AI summary is generated.</Trans>
          }
          eventType="session.enhanced"
          checked={sessionEnhanced}
          disabled={!enabled}
          onChange={(v) => {
            setSessionEnhanced(v);
            save({
              events: {
                action_items_updated: actionItems,
                session_enhanced: v,
              },
            });
          }}
        />
      </div>

      {/* Test + delivery log */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-4">
          <h3 className="text-sm font-medium">
            <Trans>Recent deliveries</Trans>
          </h3>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!canTest || test.isPending}
            onClick={() => test.mutate()}
          >
            {test.isPending ? (
              <Loader2Icon className="size-3.5 animate-spin" />
            ) : (
              <Trans>Send test event</Trans>
            )}
          </Button>
        </div>
        <DeliveryLog />
      </div>
    </div>
  );
}

function EventToggle({
  title,
  description,
  eventType,
  checked,
  disabled,
  onChange,
}: {
  title: React.ReactNode;
  description: React.ReactNode;
  eventType: string;
  checked: boolean;
  disabled: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex-1">
        <h4 className="mb-1 text-sm font-medium">{title}</h4>
        <p className="text-muted-foreground text-xs">{description}</p>
        <code className="text-muted-foreground text-[11px]">{eventType}</code>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onChange}
        disabled={disabled}
      />
    </div>
  );
}

function DeliveryLog() {
  const { data: deliveries = [], refetch } = useQuery({
    queryKey: DELIVERIES_QUERY_KEY,
    queryFn: async () => unwrap(await webhookCommands.recentDeliveries()),
  });

  // Poll while the panel is open so newly-sent test events appear.
  useEffect(() => {
    const id = setInterval(() => void refetch(), 3000);
    return () => clearInterval(id);
  }, [refetch]);

  if (deliveries.length === 0) {
    return (
      <p className="text-muted-foreground text-xs">
        <Trans>No deliveries yet.</Trans>
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      {deliveries.map((d: DeliveryRecord) => (
        <div
          key={d.id}
          className="border-border flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-xs"
        >
          <div className="flex min-w-0 items-center gap-2">
            {d.success ? (
              <CheckCircle2Icon className="size-3.5 shrink-0 text-green-600" />
            ) : (
              <XCircleIcon className="size-3.5 shrink-0 text-red-500" />
            )}
            <span className="truncate font-medium">{d.event_type}</span>
          </div>
          <div
            className={cn([
              "text-muted-foreground shrink-0",
              !d.success && "text-red-500",
            ])}
          >
            {d.status_code != null
              ? `HTTP ${d.status_code}`
              : (d.error ?? "error")}
            {d.attempts > 1 ? ` · ${d.attempts}x` : ""}
          </div>
        </div>
      ))}
    </div>
  );
}
