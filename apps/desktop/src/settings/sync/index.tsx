import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  CheckIcon,
  CopyIcon,
  Loader2Icon,
  MonitorSmartphoneIcon,
  PlusIcon,
  Trash2Icon,
} from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useId, useRef, useState } from "react";

import {
  syncAddPeer,
  syncListPeers,
  syncRemovePeer,
  syncStart,
  syncStatus,
  syncStop,
  syncThisDevice,
  type SyncPeer,
  type SyncStatusResult,
} from "@hypr/plugin-db";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue, useStoredSettingValue } from "~/settings/queries";

const STATUS_QUERY_KEY = ["settings", "sync", "status"] as const;
const THIS_DEVICE_QUERY_KEY = ["settings", "sync", "this-device"] as const;
const PEERS_QUERY_KEY = ["settings", "sync", "peers"] as const;

// z-base-32 (RFC-ish, no 0/l/v/2 etc.) — matches sync_p2p::Fingerprint's
// alphabet. Grouped display form adds dashes every 4 chars; both are valid
// input here since sync_add_peer accepts either.
const Z_BASE_32_ALPHABET = "ybndrfg8ejkmcpqxot1uwisza345h769";
const FINGERPRINT_LENGTH = 52;

export function normalizeFingerprintInput(raw: string): string {
  return raw.replace(/[\s-]/g, "").toLowerCase();
}

export function isValidFingerprint(raw: string): boolean {
  const compact = normalizeFingerprintInput(raw);
  if (compact.length !== FINGERPRINT_LENGTH) {
    return false;
  }
  return [...compact].every((char) => Z_BASE_32_ALPHABET.includes(char));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function SettingsSync() {
  const { t } = useLingui();
  const { data, isLoading, error } = useQuery({
    queryKey: STATUS_QUERY_KEY,
    queryFn: syncStatus,
  });

  if (error) {
    throw error;
  }

  if (isLoading || !data) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label={t`Loading sync status`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  if (data.kind === "unavailable") {
    return <SyncUnavailable />;
  }

  return <SyncSettingsContent status={data} />;
}

function SyncUnavailable() {
  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Devices</Trans>} />
      <div className="border-border text-muted-foreground flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center">
        <MonitorSmartphoneIcon className="text-muted-foreground/70 size-6" />
        <p className="text-foreground text-sm font-medium">
          <Trans>Device sync isn't available in this build</Trans>
        </p>
        <p className="max-w-sm text-xs">
          <Trans>
            Direct device-to-device sync isn't included in this build of Notare.
          </Trans>
        </p>
      </div>
    </div>
  );
}

function SyncSettingsContent({
  status,
}: {
  status: Extract<SyncStatusResult, { kind: "live" }>;
}) {
  const queryClient = useQueryClient();

  const { value: syncEnabledValue } = useStoredSettingValue("sync_enabled");
  const syncEnabled = syncEnabledValue ?? false;
  const setSyncEnabled = useSetSettingValue("sync_enabled");

  const toggleSync = useMutation({
    mutationFn: (next: boolean) => (next ? syncStart() : syncStop()),
    // Refetch the status on both outcomes — a failed start/stop can still
    // change what the agent is doing (e.g. a stop that partially tore down
    // cloudsync before failing), so the status line must never keep showing
    // a stale pre-attempt read.
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY });
    },
    // The setting is set optimistically in handleToggle so the switch
    // responds immediately; if the agent fails to start/stop, revert it so
    // the persisted setting — and the pairing UI it gates — track what the
    // agent actually did rather than what was merely requested.
    onError: (_error, next) => {
      setSyncEnabled(!next);
    },
  });

  const handleToggle = (next: boolean) => {
    setSyncEnabled(next);
    toggleSync.mutate(next);
  };

  const thisDeviceQuery = useQuery({
    queryKey: THIS_DEVICE_QUERY_KEY,
    queryFn: syncThisDevice,
  });

  const peersQuery = useQuery({
    queryKey: PEERS_QUERY_KEY,
    queryFn: syncListPeers,
  });

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Devices</Trans>} />

      <SyncEnableToggle
        enabled={syncEnabled}
        onChange={handleToggle}
        isPending={toggleSync.isPending}
        startingUp={toggleSync.isPending && toggleSync.variables === true}
        error={toggleSync.isError ? toggleSync.error : null}
        errorWasStarting={toggleSync.variables === true}
      />

      <p className="text-muted-foreground text-xs">
        <Trans>
          Pair another one of your devices to sync sessions between them
          directly, without going through a server. Both devices need to add
          each other's fingerprint before they'll sync.
        </Trans>
      </p>

      <SyncStatusLine status={status} />

      <motion.div
        animate={{ opacity: syncEnabled ? 1 : 0.5 }}
        transition={{ duration: 0.2 }}
        aria-disabled={!syncEnabled}
        className={cn([
          "flex flex-col gap-6",
          !syncEnabled && "pointer-events-none",
        ])}
      >
        {!syncEnabled && (
          <p className="text-muted-foreground text-xs italic">
            <Trans>
              Sync is off, so pairing won't do anything yet. Turn it on above
              first.
            </Trans>
          </p>
        )}

        <ThisDeviceSection
          fingerprint={thisDeviceQuery.data}
          isLoading={thisDeviceQuery.isLoading}
          error={thisDeviceQuery.error}
          disabled={!syncEnabled}
        />

        <AddDeviceForm
          disabled={!syncEnabled}
          onAdded={() => {
            void queryClient.invalidateQueries({ queryKey: PEERS_QUERY_KEY });
          }}
        />

        <PairedDevicesList
          peers={peersQuery.data ?? []}
          isLoading={peersQuery.isLoading}
          disabled={!syncEnabled}
        />
      </motion.div>
    </div>
  );
}

function SyncEnableToggle({
  enabled,
  onChange,
  isPending,
  startingUp,
  error,
  errorWasStarting,
}: {
  enabled: boolean;
  onChange: (next: boolean) => void;
  isPending: boolean;
  startingUp: boolean;
  error: unknown;
  errorWasStarting: boolean;
}) {
  const { t } = useLingui();
  const titleId = useId();
  const descriptionId = useId();

  return (
    <div className="border-border bg-muted/30 flex flex-col gap-3 rounded-lg border p-4">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 id={titleId} className="text-sm font-medium">
            <Trans>Enable device sync (experimental)</Trans>
          </h3>
          <p id={descriptionId} className="text-muted-foreground mt-1 text-xs">
            <Trans>
              Off by default. Turning this on starts a background sync agent
              that publishes this device's id and network address to Number 0
              (n0.computer)'s public discovery service for as long as it runs,
              so your other devices can find it. No note content or
              paired-device information is ever published — only where this
              device can be reached. Turning it off immediately stops
              advertising this device.
            </Trans>
          </p>
        </div>
        <Switch
          checked={enabled}
          onCheckedChange={onChange}
          disabled={isPending}
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
        />
      </div>

      {isPending && (
        <p className="text-muted-foreground flex items-center gap-1.5 text-xs">
          <Loader2Icon className="size-3 animate-spin" />
          {startingUp ? t`Starting sync…` : t`Stopping sync…`}
        </p>
      )}

      {error != null && (
        <p className="text-xs text-red-500">
          {errorWasStarting
            ? t`Couldn't start sync, so it's been turned back off: ${errorMessage(error)}`
            : t`Couldn't stop sync cleanly, so it's been turned back on: ${errorMessage(error)}`}
        </p>
      )}
    </div>
  );
}

function SyncStatusLine({
  status,
}: {
  status: Extract<SyncStatusResult, { kind: "live" }>;
}) {
  const { t } = useLingui();

  return (
    <div className="text-muted-foreground flex flex-wrap items-center gap-1.5 text-xs">
      <span
        aria-hidden="true"
        className={cn([
          "size-1.5 rounded-full",
          status.running ? "bg-green-500" : "bg-muted-foreground/40",
        ])}
      />
      <span>{status.running ? t`Sync running` : t`Sync idle`}</span>
      {status.lastSyncAtMs != null && (
        <>
          <span aria-hidden="true">·</span>
          <span>
            {t`Last synced ${formatDistanceToNow(new Date(status.lastSyncAtMs), { addSuffix: true })}`}
          </span>
        </>
      )}
      {status.hasUnsentChanges && (
        <>
          <span aria-hidden="true">·</span>
          <span>
            <Trans>Unsent changes pending</Trans>
          </span>
        </>
      )}
    </div>
  );
}

function ThisDeviceSection({
  fingerprint,
  isLoading,
  error,
  disabled,
}: {
  fingerprint: string | undefined;
  isLoading: boolean;
  error: unknown;
  disabled: boolean;
}) {
  const { t } = useLingui();
  const [copied, setCopied] = useState(false);
  const copyResetRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetRef.current) {
        clearTimeout(copyResetRef.current);
      }
    };
  }, []);

  const handleCopy = async () => {
    if (!fingerprint) {
      return;
    }
    try {
      await writeText(fingerprint);
    } catch {
      // Fall back to the browser clipboard when the plugin is unavailable.
      await navigator.clipboard.writeText(fingerprint);
    }
    setCopied(true);
    sonnerToast.success(t`Copied to clipboard`);
    if (copyResetRef.current) {
      clearTimeout(copyResetRef.current);
    }
    copyResetRef.current = setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">
        <Trans>This device</Trans>
      </h3>
      <p className="text-muted-foreground text-xs">
        <Trans>Share this fingerprint with another device to pair it.</Trans>
      </p>
      <div className="flex items-center gap-2">
        <code className="border-border bg-muted min-w-0 flex-1 truncate rounded-lg border px-3 py-2 font-mono text-xs">
          {isLoading
            ? t`Loading…`
            : (fingerprint ?? (error ? errorMessage(error) : ""))}
        </code>
        <Button
          type="button"
          variant="outline"
          size="icon"
          disabled={!fingerprint || disabled}
          onClick={() => void handleCopy()}
          aria-label={t`Copy fingerprint`}
        >
          {copied ? (
            <CheckIcon className="size-4" />
          ) : (
            <CopyIcon className="size-4" />
          )}
        </Button>
      </div>
    </div>
  );
}

function AddDeviceForm({
  onAdded,
  disabled,
}: {
  onAdded: () => void;
  disabled: boolean;
}) {
  const { t } = useLingui();

  const addPeer = useMutation({
    mutationFn: ({
      fingerprint,
      label,
    }: {
      fingerprint: string;
      label: string;
    }) => syncAddPeer(fingerprint, label),
    onSuccess: () => {
      form.reset();
      onAdded();
    },
  });

  const form = useForm({
    defaultValues: { fingerprint: "", label: "" },
    validators: {
      onChange: ({ value }) => {
        if (
          value.fingerprint.length > 0 &&
          !isValidFingerprint(value.fingerprint)
        ) {
          return {
            fields: {
              fingerprint: t`Enter a valid 52-character fingerprint`,
            },
          };
        }
        return undefined;
      },
    },
    onSubmit: async ({ value }) => {
      // Submit the same normalized form the validator checked — sending the
      // raw (dashed / possibly mixed-case) input risks a mismatch with what
      // was actually validated as a 52-char z-base-32 fingerprint.
      await addPeer.mutateAsync({
        fingerprint: normalizeFingerprintInput(value.fingerprint),
        label: value.label.trim(),
      });
    },
  });

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">
        <Trans>Add a device</Trans>
      </h3>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Paste the fingerprint shown on the other device's Devices settings
          page.
        </Trans>
      </p>
      <form
        className="flex flex-col gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          e.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="fingerprint">
          {(field) => (
            <div className="flex flex-col gap-1">
              <Input
                value={field.state.value}
                onChange={(e) => field.handleChange(e.target.value)}
                placeholder="abcd-efgh-ijkl-..."
                aria-label={t`Peer fingerprint`}
                className="font-mono text-xs"
                disabled={disabled}
              />
              {field.state.meta.errors.length > 0 && (
                <p className="text-xs text-red-500">
                  {String(field.state.meta.errors[0])}
                </p>
              )}
            </div>
          )}
        </form.Field>
        <form.Field name="label">
          {(field) => (
            <Input
              value={field.state.value}
              onChange={(e) => field.handleChange(e.target.value)}
              placeholder={t`Label (optional), e.g. "Work laptop"`}
              aria-label={t`Device label`}
              disabled={disabled}
            />
          )}
        </form.Field>

        {addPeer.error && (
          <p className="text-xs text-red-500">{errorMessage(addPeer.error)}</p>
        )}

        <form.Subscribe
          selector={(state) =>
            [state.canSubmit, state.values.fingerprint] as const
          }
        >
          {([canSubmit, fingerprintValue]) => (
            <Button
              type="submit"
              variant="outline"
              className="self-start"
              disabled={
                disabled ||
                !canSubmit ||
                !isValidFingerprint(fingerprintValue) ||
                addPeer.isPending
              }
            >
              {addPeer.isPending ? (
                <Loader2Icon className="size-3.5 animate-spin" />
              ) : (
                <PlusIcon className="size-3.5" />
              )}
              <Trans>Add device</Trans>
            </Button>
          )}
        </form.Subscribe>
      </form>
    </div>
  );
}

function PairedDevicesList({
  peers,
  isLoading,
  disabled,
}: {
  peers: SyncPeer[];
  isLoading: boolean;
  disabled: boolean;
}) {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [pendingRemove, setPendingRemove] = useState<SyncPeer | null>(null);

  const removePeer = useMutation({
    mutationFn: (nodeId: string) => syncRemovePeer(nodeId),
    onSuccess: () => {
      setPendingRemove(null);
      void queryClient.invalidateQueries({ queryKey: PEERS_QUERY_KEY });
    },
  });

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">
        <Trans>Paired devices</Trans>
      </h3>

      {isLoading ? (
        <Loader2Icon className="text-muted-foreground size-4 animate-spin" />
      ) : peers.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          <Trans>No devices paired yet.</Trans>
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {peers.map((peer) => (
            <li
              key={peer.nodeId}
              className="border-border flex items-center gap-3 rounded-lg border p-3"
            >
              <MonitorSmartphoneIcon className="text-muted-foreground size-4 shrink-0" />
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <p className="truncate text-sm font-medium">
                  {peer.label || t`Unnamed device`}
                </p>
                <div className="text-muted-foreground flex min-w-0 items-center gap-1.5 font-mono text-xs">
                  <span className="truncate">{peer.fingerprint}</span>
                  <span aria-hidden="true" className="shrink-0">
                    ·
                  </span>
                  <span className="shrink-0 font-sans">
                    {peer.lastSeen > 0
                      ? t`Last seen ${formatDistanceToNow(new Date(peer.lastSeen * 1000), { addSuffix: true })}`
                      : t`Never connected`}
                  </span>
                </div>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="shrink-0 text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300"
                disabled={disabled}
                onClick={() => setPendingRemove(peer)}
              >
                <Trash2Icon className="size-3.5" />
                <Trans>Remove</Trans>
              </Button>
            </li>
          ))}
        </ul>
      )}

      <Dialog
        open={pendingRemove !== null}
        onOpenChange={(open) => {
          if (!open && !removePeer.isPending) {
            setPendingRemove(null);
          }
        }}
      >
        <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
          <DialogHeader className="items-center gap-2 px-5 pt-6 text-center sm:text-center">
            <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
              <Trans>Remove this device?</Trans>
            </DialogTitle>
            <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
              <Trans>
                "{pendingRemove?.label || t`Unnamed device`}" will no longer be
                able to sync with this device. You can pair it again later.
              </Trans>
            </DialogDescription>
          </DialogHeader>

          {removePeer.error && (
            <p className="mx-4 mt-3 text-center text-xs text-red-500">
              {errorMessage(removePeer.error)}
            </p>
          )}

          <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
            <Button
              variant="ghost"
              className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-4 text-xs font-medium shadow-none"
              onClick={() => setPendingRemove(null)}
              disabled={removePeer.isPending}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button
              variant="destructive"
              className="h-8 rounded-full px-4 text-xs font-medium shadow-sm"
              onClick={() => {
                if (pendingRemove) {
                  removePeer.mutate(pendingRemove.nodeId);
                }
              }}
              disabled={removePeer.isPending}
            >
              {removePeer.isPending ? t`Removing...` : t`Remove`}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
