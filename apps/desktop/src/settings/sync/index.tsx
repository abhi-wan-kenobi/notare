import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  CheckIcon,
  CopyIcon,
  FolderOpenIcon,
  Loader2Icon,
  Link2Icon,
  MonitorSmartphoneIcon,
  PlusIcon,
  QrCodeIcon,
  ShieldCheckIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useId, useRef, useState } from "react";

import {
  execute,
  pairingConfirm,
  pairingCreateOffer,
  pairingJoin,
  pairingStatus,
  syncAddPeer,
  syncListPeers,
  syncRevokePeer,
  syncStart,
  syncStatus,
  syncStop,
  syncThisDevice,
  type PairingJoinResult,
  type PairingOfferResult,
  type SyncPeer,
  type SyncStatusResult,
} from "@hypr/plugin-db";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { Badge } from "@hypr/ui/components/ui/badge";
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

import { usePendingPairLink } from "./pending-pair-link";

import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue, useStoredSettingValue } from "~/settings/queries";

const STATUS_QUERY_KEY = ["settings", "sync", "status"] as const;
const THIS_DEVICE_QUERY_KEY = ["settings", "sync", "this-device"] as const;
const PEERS_QUERY_KEY = ["settings", "sync", "peers"] as const;
const CRR_CONVERTED_QUERY_KEY = ["settings", "sync", "crr-converted"] as const;
const PAIRING_STATUS_QUERY_KEY = [
  "settings",
  "sync",
  "pairing-status",
] as const;

// How often the offer dialog polls pairing_status() for an incoming join —
// frequent enough to feel live, rare enough not to hammer the agent.
const PAIRING_POLL_MS = 1500;

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

// `start_sync` runs `crsql_as_crr` on every synced table in one go, and each
// conversion adds a `<table>__crsql_clock` companion. Any such table means the
// one-time storage change has already happened on this database.
async function hasConvertedSyncTables(): Promise<boolean> {
  const rows = await execute<{ n: number }>(
    "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name LIKE '%\\_\\_crsql\\_clock' ESCAPE '\\'",
  );
  return (rows[0]?.n ?? 0) > 0;
}

function formatCountdown(secondsRemaining: number): string {
  if (secondsRemaining <= 0) {
    return "0:00";
  }
  const m = Math.floor(secondsRemaining / 60);
  const s = secondsRemaining % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
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
  const incoming = usePendingPairLink((state) => state.link);
  const clearInbox = usePendingPairLink((state) => state.clear);
  const [droppedLink, setDroppedLink] = useState(false);

  // Nothing here can join; drop the link so it can't linger in memory.
  useEffect(() => {
    if (incoming) {
      setDroppedLink(true);
      clearInbox();
    }
  }, [incoming, clearInbox]);

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Devices</Trans>} />
      {droppedLink && (
        <p
          role="alert"
          className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
        >
          <Trans>
            A pairing link was opened, but device sync isn't available in this
            build, so it can't be used here.
          </Trans>
        </p>
      )}
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
    // the sync session before failing), so the status line must never keep
    // showing a stale pre-attempt read.
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY });
      void queryClient.invalidateQueries({
        queryKey: CRR_CONVERTED_QUERY_KEY,
      });
    },
    // The setting is set optimistically in handleToggle so the switch
    // responds immediately; if the agent fails to start/stop, revert it so
    // the persisted setting — and the pairing UI it gates — track what the
    // agent actually did rather than what was merely requested.
    onError: (_error, next) => {
      setSyncEnabled(!next);
    },
  });

  // Unknown (loading or failed) counts as "not converted": the backup prompt
  // is the safe default before anything rewrites the notes tables.
  const crrConvertedQuery = useQuery({
    queryKey: CRR_CONVERTED_QUERY_KEY,
    queryFn: hasConvertedSyncTables,
  });
  const [confirmingFirstEnable, setConfirmingFirstEnable] = useState(false);

  const applyToggle = (next: boolean) => {
    setSyncEnabled(next);
    toggleSync.mutate(next);
  };

  const handleToggle = (next: boolean) => {
    if (next && crrConvertedQuery.data !== true) {
      setConfirmingFirstEnable(true);
      return;
    }
    applyToggle(next);
  };

  const thisDeviceQuery = useQuery({
    queryKey: THIS_DEVICE_QUERY_KEY,
    queryFn: syncThisDevice,
  });

  const peersQuery = useQuery({
    queryKey: PEERS_QUERY_KEY,
    queryFn: syncListPeers,
  });

  const invalidatePeers = () => {
    void queryClient.invalidateQueries({ queryKey: PEERS_QUERY_KEY });
  };

  const incomingLink = usePendingPairLink((state) => state.link);

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle
        title={
          <span className="flex items-center gap-3">
            <Trans>Devices</Trans>
            <Badge
              variant="outline"
              className="border-amber-500/30 bg-amber-500/10 font-sans text-[10px] text-amber-600 dark:text-amber-400"
            >
              <Trans>Preview</Trans>
            </Badge>
          </span>
        }
      />

      <div className="text-muted-foreground flex flex-col gap-2 text-xs">
        <p>
          <Trans>
            Device sync is a preview. It keeps your notes in sync between your
            own devices directly, without going through a server. Every device
            you pair must run Notare 0.6.8 or later.
          </Trans>
        </p>
        <p>
          <Trans>
            It stays off until you turn it on below. Removing a paired device is
            permanent: it can't reconnect with an old link and has to be paired
            again from scratch.
          </Trans>
        </p>
      </div>

      <SyncEnableToggle
        enabled={syncEnabled}
        onChange={handleToggle}
        isPending={toggleSync.isPending}
        startingUp={toggleSync.isPending && toggleSync.variables === true}
        error={toggleSync.isError ? toggleSync.error : null}
        errorWasStarting={toggleSync.variables === true}
      />

      <FirstEnableDialog
        open={confirmingFirstEnable}
        onOpenChange={setConfirmingFirstEnable}
        onContinue={() => {
          setConfirmingFirstEnable(false);
          applyToggle(true);
        }}
      />

      <SyncStatusLine status={status} />

      {incomingLink && !syncEnabled && (
        <p
          role="alert"
          className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
        >
          <Trans>
            A pairing link was opened, but sync is off. Turn it on above to join
            with it.
          </Trans>
        </p>
      )}

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

        <div className="flex flex-col gap-3 sm:flex-row">
          <PairDeviceSection
            disabled={!syncEnabled}
            onPaired={invalidatePeers}
          />
          <JoinWithLinkSection
            disabled={!syncEnabled}
            acceptIncoming={syncEnabled && !toggleSync.isPending}
            onJoined={invalidatePeers}
          />
        </div>

        <PairedDevicesList
          peers={peersQuery.data ?? []}
          isLoading={peersQuery.isLoading}
          disabled={!syncEnabled}
        />

        <Accordion type="single" collapsible className="flex flex-col">
          <AccordionItem value="advanced" className="border-none">
            <AccordionTrigger className="text-muted-foreground py-2 text-xs font-medium hover:no-underline">
              <Trans>Advanced: manual fingerprint pairing</Trans>
            </AccordionTrigger>
            <AccordionContent className="flex flex-col gap-6 pt-2">
              <ThisDeviceSection
                fingerprint={thisDeviceQuery.data}
                isLoading={thisDeviceQuery.isLoading}
                error={thisDeviceQuery.error}
                disabled={!syncEnabled}
              />

              <AddDeviceForm
                disabled={!syncEnabled}
                onAdded={invalidatePeers}
              />
            </AccordionContent>
          </AccordionItem>
        </Accordion>
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
            <Trans>Enable device sync</Trans>
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

function FirstEnableDialog({
  open,
  onOpenChange,
  onContinue,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onContinue: () => void;
}) {
  const openDataFolder = useMutation({
    mutationFn: async () => {
      const base = await settingsCommands.globalBase();
      if (base.status === "error") {
        throw new Error(base.error);
      }
      const opened = await openerCommands.openPath(base.data, null);
      if (opened.status === "error") {
        throw new Error(opened.error);
      }
    },
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          openDataFolder.reset();
        }
        onOpenChange(next);
      }}
    >
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>
            <Trans>Back up before turning on sync?</Trans>
          </DialogTitle>
          <DialogDescription>
            <Trans>
              This changes how Notare stores notes so they can sync. Back up
              first?
            </Trans>
          </DialogDescription>
        </DialogHeader>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Your sessions, notes, transcripts, action items and tags are
            converted to a sync-ready format the first time sync starts. Turning
            sync off later doesn't undo this. To back up, open the data folder,
            quit Notare, and copy the folder somewhere safe.
          </Trans>
        </p>
        {openDataFolder.isError && (
          <p className="text-xs text-red-500">
            {errorMessage(openDataFolder.error)}
          </p>
        )}
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => openDataFolder.mutate()}
            disabled={openDataFolder.isPending}
          >
            <FolderOpenIcon className="size-4" />
            <Trans>Open data folder</Trans>
          </Button>
          <Button
            onClick={() => {
              openDataFolder.reset();
              onContinue();
            }}
          >
            <Trans>Continue</Trans>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
      {!status.engineLoaded && (
        <>
          <span aria-hidden="true">·</span>
          <span>
            <Trans>Sync engine not loaded on this build</Trans>
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
  const copyResetRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    return () => clearTimeout(copyResetRef.current);
  }, []);

  const handleCopy = async () => {
    if (!fingerprint) {
      return;
    }
    try {
      await writeText(fingerprint);
    } catch {
      // Fall back to the browser clipboard when the plugin is unavailable.
      try {
        await navigator.clipboard.writeText(fingerprint);
      } catch {
        sonnerToast.error(t`Couldn't copy to clipboard`);
        return;
      }
    }
    setCopied(true);
    sonnerToast.success(t`Copied to clipboard`);
    clearTimeout(copyResetRef.current);
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

// ---------------------------------------------------------------------------
// SYNC-6: QR / link pairing
// ---------------------------------------------------------------------------

/// Shared SAS confirmation view: shows the peer's name and the 6-digit code,
/// with Confirm/Reject buttons. Used both by the offerer (after someone
/// joins their offer) and the joiner (after dialing an offer).
function SasConfirm({
  deviceName,
  sas,
  onConfirm,
  onReject,
  isPending,
}: {
  deviceName: string;
  sas: string;
  onConfirm: () => void;
  onReject: () => void;
  isPending: boolean;
}) {
  return (
    <div className="flex flex-col items-center gap-4 py-2">
      <ShieldCheckIcon className="text-muted-foreground size-8" />
      <div className="text-center">
        <p className="text-sm font-medium">
          <Trans>Confirm with {deviceName || "the other device"}</Trans>
        </p>
        <p className="text-muted-foreground mt-1 text-xs">
          <Trans>Make sure this code matches on both devices.</Trans>
        </p>
      </div>
      <p
        className="font-mono text-3xl font-semibold tracking-[0.3em]"
        data-testid="sas-code"
      >
        {sas}
      </p>
      <div className="flex w-full gap-2">
        <Button
          type="button"
          variant="ghost"
          className="flex-1"
          onClick={onReject}
          disabled={isPending}
        >
          <XIcon className="size-3.5" />
          <Trans>Reject</Trans>
        </Button>
        <Button
          type="button"
          className="flex-1"
          onClick={onConfirm}
          disabled={isPending}
        >
          {isPending ? (
            <Loader2Icon className="size-3.5 animate-spin" />
          ) : (
            <CheckIcon className="size-3.5" />
          )}
          <Trans>Confirm</Trans>
        </Button>
      </div>
    </div>
  );
}

function PairDeviceSection({
  disabled,
  onPaired,
}: {
  disabled: boolean;
  onPaired: () => void;
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [offer, setOffer] = useState<PairingOfferResult | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [copied, setCopied] = useState(false);
  const copyResetRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    return () => clearTimeout(copyResetRef.current);
  }, []);

  const createOffer = useMutation({
    mutationFn: pairingCreateOffer,
    onSuccess: (result) => setOffer(result),
  });

  const createOfferRef = useRef(createOffer.mutate);
  createOfferRef.current = createOffer.mutate;

  useEffect(() => {
    if (open) {
      setOffer(null);
      createOfferRef.current();
    }
  }, [open]);

  // Tick every second while the dialog is open, to drive the countdown.
  useEffect(() => {
    if (!open) {
      return;
    }
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [open]);

  const secondsRemaining = offer
    ? Math.max(0, offer.expiresAt - Math.floor(now / 1000))
    : 0;
  const expired = offer != null && secondsRemaining <= 0;

  // Poll pairing_status() while the dialog is open and the offer hasn't
  // expired, watching for a joiner's proof to produce a pending confirmation.
  const statusQuery = useQuery({
    queryKey: PAIRING_STATUS_QUERY_KEY,
    queryFn: pairingStatus,
    enabled: open && offer != null && !expired,
    refetchInterval: PAIRING_POLL_MS,
  });

  const pending = statusQuery.data?.pending[0];

  const confirmMutation = useMutation({
    mutationFn: ({ deviceId, accept }: { deviceId: string; accept: boolean }) =>
      pairingConfirm(deviceId, accept),
    onSuccess: (_void, variables) => {
      if (variables.accept) {
        sonnerToast.success(t`Device paired`);
        onPaired();
      }
      setOpen(false);
    },
  });

  const handleCopyLink = async () => {
    if (!offer) {
      return;
    }
    try {
      await writeText(offer.link);
    } catch {
      try {
        await navigator.clipboard.writeText(offer.link);
      } catch {
        sonnerToast.error(t`Couldn't copy to clipboard`);
        return;
      }
    }
    setCopied(true);
    sonnerToast.success(t`Link copied`);
    clearTimeout(copyResetRef.current);
    copyResetRef.current = setTimeout(() => setCopied(false), 1500);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Button
        type="button"
        variant="outline"
        className="flex-1"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <QrCodeIcon className="size-3.5" />
        <Trans>Pair a device</Trans>
      </Button>

      <DialogContent className="max-w-xs">
        <DialogHeader>
          <DialogTitle>
            {pending ? (
              <Trans>Confirm pairing</Trans>
            ) : (
              <Trans>Pair a device</Trans>
            )}
          </DialogTitle>
          <DialogDescription>
            {pending ? (
              <Trans>
                Someone scanned your code. Compare the numbers below.
              </Trans>
            ) : (
              <Trans>Scan this on the other device, or copy the link.</Trans>
            )}
          </DialogDescription>
        </DialogHeader>

        {pending ? (
          <SasConfirm
            deviceName={pending.name}
            sas={pending.sas}
            isPending={confirmMutation.isPending}
            onConfirm={() =>
              confirmMutation.mutate({
                deviceId: pending.deviceId,
                accept: true,
              })
            }
            onReject={() =>
              confirmMutation.mutate({
                deviceId: pending.deviceId,
                accept: false,
              })
            }
          />
        ) : createOffer.isError ? (
          <div className="flex flex-col items-center gap-3 py-4">
            <p className="text-xs text-red-500">
              {errorMessage(createOffer.error)}
            </p>
            <Button
              type="button"
              variant="outline"
              onClick={() => createOffer.mutate()}
            >
              <Trans>Try again</Trans>
            </Button>
          </div>
        ) : createOffer.isPending || !offer ? (
          <div className="flex min-h-48 items-center justify-center">
            <Loader2Icon className="text-muted-foreground size-5 animate-spin" />
          </div>
        ) : expired ? (
          <div className="flex flex-col items-center gap-3 py-4">
            <p className="text-muted-foreground text-xs">
              <Trans>This code expired.</Trans>
            </p>
            <Button
              type="button"
              variant="outline"
              onClick={() => createOffer.mutate()}
            >
              <Trans>Generate a new code</Trans>
            </Button>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <img
              // Generated server-side by the `qrcode` crate from the offer
              // link we also render as text below. Rendered as an <img> data
              // URI (never dangerouslySetInnerHTML) so a malicious SVG can't
              // reach the DOM or execute script.
              src={`data:image/svg+xml;utf8,${encodeURIComponent(offer.qrSvg)}`}
              alt={t`Pairing QR code`}
              className="size-40"
            />
            <div className="flex w-full items-center gap-2">
              <code className="border-border bg-muted min-w-0 flex-1 truncate rounded-lg border px-3 py-2 font-mono text-xs">
                {offer.link}
              </code>
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={() => void handleCopyLink()}
                aria-label={t`Copy link`}
              >
                {copied ? (
                  <CheckIcon className="size-4" />
                ) : (
                  <CopyIcon className="size-4" />
                )}
              </Button>
            </div>
            <p className="text-muted-foreground text-xs">
              <Trans>Expires in {formatCountdown(secondsRemaining)}</Trans>
            </p>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function JoinWithLinkSection({
  disabled,
  acceptIncoming,
  onJoined,
}: {
  disabled: boolean;
  acceptIncoming: boolean;
  onJoined: () => void;
}) {
  const { t } = useLingui();
  const [link, setLink] = useState("");
  const [outcome, setOutcome] = useState<PairingJoinResult | null>(null);

  const joinMutation = useMutation({
    mutationFn: pairingJoin,
    onSuccess: (result) => setOutcome(result),
  });

  // A notare://pair# link opened from the OS: fill the field and start the
  // join, which ends in the same 6-digit confirmation as a pasted link.
  const incoming = usePendingPairLink((state) => state.link);
  const clearInbox = usePendingPairLink((state) => state.clear);
  const startJoin = joinMutation.mutate;
  useEffect(() => {
    if (!incoming || !acceptIncoming) {
      return;
    }
    clearInbox();
    setOutcome(null);
    setLink(incoming);
    startJoin(incoming);
  }, [incoming, acceptIncoming, clearInbox, startJoin]);

  const addPeer = useMutation({
    mutationFn: ({
      fingerprint,
      label,
    }: {
      fingerprint: string;
      label: string;
    }) => syncAddPeer(fingerprint, label),
    onSuccess: () => {
      sonnerToast.success(t`Device paired`);
      setOutcome(null);
      setLink("");
      onJoined();
    },
  });

  return (
    <>
      <form
        className="flex flex-1 gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (link.trim().length > 0) {
            joinMutation.mutate(link.trim());
          }
        }}
      >
        <Input
          value={link}
          onChange={(e) => setLink(e.target.value)}
          placeholder={t`Paste a pairing link…`}
          aria-label={t`Pairing link`}
          disabled={disabled}
          className="font-mono text-xs"
        />
        <Button
          type="submit"
          variant="outline"
          disabled={
            disabled || link.trim().length === 0 || joinMutation.isPending
          }
        >
          {joinMutation.isPending ? (
            <Loader2Icon className="size-3.5 animate-spin" />
          ) : (
            <Link2Icon className="size-3.5" />
          )}
          <Trans>Join</Trans>
        </Button>
      </form>

      {joinMutation.isError && (
        <p className="text-xs text-red-500">
          {errorMessage(joinMutation.error)}
        </p>
      )}

      <Dialog
        open={outcome !== null}
        onOpenChange={(nextOpen) => {
          if (!nextOpen && !addPeer.isPending) {
            setOutcome(null);
          }
        }}
      >
        <DialogContent className="max-w-xs">
          <DialogHeader>
            <DialogTitle>
              <Trans>Confirm pairing</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>Compare the numbers shown on both devices.</Trans>
            </DialogDescription>
          </DialogHeader>
          {outcome && (
            <SasConfirm
              deviceName={outcome.deviceName}
              sas={outcome.sas}
              isPending={addPeer.isPending}
              onConfirm={() =>
                addPeer.mutate({
                  fingerprint: outcome.deviceId,
                  label: outcome.deviceName,
                })
              }
              onReject={() => setOutcome(null)}
            />
          )}
          {addPeer.error && (
            <p className="text-xs text-red-500">
              {errorMessage(addPeer.error)}
            </p>
          )}
        </DialogContent>
      </Dialog>
    </>
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

  // Revoke (not a soft remove): closes the §12-class gap where a stale
  // pairing offer or a peer's locally-cached allowlist entry could let a
  // removed device re-add itself. See sync_p2p::peers::PeerStore::revoke.
  const removePeer = useMutation({
    mutationFn: (nodeId: string) => syncRevokePeer(nodeId),
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
                able to sync with this device, and can't re-pair with a saved
                link. You'd need to pair it again from scratch.
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
