import { Trans, useLingui } from "@lingui/react/macro";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { platform } from "@tauri-apps/plugin-os";
import { AlertCircleIcon, Trash2Icon } from "lucide-react";
import { type ReactNode, useEffect, useId, useState } from "react";

import type { PermissionStatus } from "@hypr/plugin-permissions";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@hypr/utils";

import {
  clearDictationHistory,
  deleteDictationHistoryEntry,
  type DictationHistoryEntry,
  useDictationHistory,
} from "~/dictation/history";
import {
  DictationOrb,
  type DictationOrbVariant,
  normalizeOrbVariant,
  ORB_VARIANT_ORDER,
  ORB_VARIANT_REGISTRY,
} from "~/dictation/orb";
import { normalizeOutputMode } from "~/dictation/output-mode";
import { normalizeCleanupMode } from "~/dictation/finalize";
import { ShortcutRecorderRow } from "~/settings/dictation/shortcut-recorder";
import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue } from "~/settings/queries";
import { SETTING_DEFINITIONS } from "~/settings/schema";
import { useConfigValues } from "~/shared/config";
import { usePermission } from "~/shared/hooks/usePermissions";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * Dictation settings: the persistent dictation orb that types recognized
 * speech into whichever app has keyboard focus. Runs on every platform since
 * #31 - macOS reaches parity through this same webview orb instead of its
 * unfinished native panel.
 */
export function SettingsDictation() {
  const {
    dictation_enabled,
    dictation_shortcut,
    dictation_output_mode,
    dictation_paste_at_cursor,
    dictation_cleanup,
    dictation_orb_variant,
    dictation_caption,
  } = useConfigValues([
    "dictation_enabled",
    "dictation_shortcut",
    "dictation_output_mode",
    "dictation_paste_at_cursor",
    "dictation_cleanup",
    "dictation_orb_variant",
    "dictation_caption",
  ] as const);
  const setEnabled = useSetSettingValue("dictation_enabled");
  const setShortcut = useSetSettingValue("dictation_shortcut");
  const setOutputMode = useSetSettingValue("dictation_output_mode");
  const setPasteAtCursor = useSetSettingValue("dictation_paste_at_cursor");
  const setCleanup = useSetSettingValue("dictation_cleanup");
  const setOrbVariant = useSetSettingValue("dictation_orb_variant");
  const setCaption = useSetSettingValue("dictation_caption");

  const outputMode = normalizeOutputMode(dictation_output_mode);

  const { conn, isLocalModel } = useSTTConnection();
  const modelReady = isLocalModel && !!conn;

  const isMacos = platform() === "macos";
  const accessibility = usePermission("accessibility");

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Dictation</Trans>} />

      <section>
        <div className="flex flex-col gap-4">
          <SettingRow
            title={<Trans>Show dictation orb</Trans>}
            description={
              <Trans>
                Keep a small always-on-top orb on screen. Click it, or press the
                shortcut, to start typing what you say into the focused app.
              </Trans>
            }
            checked={dictation_enabled}
            onChange={setEnabled}
          />
          {isMacos && dictation_enabled ? (
            <MacosAccessibilityHint
              status={accessibility.status}
              isPending={accessibility.isPending}
              onRequest={accessibility.request}
              onOpen={accessibility.open}
            />
          ) : null}
          <ShortcutRecorderRow
            value={dictation_shortcut}
            defaultValue={SETTING_DEFINITIONS.dictation_shortcut.default}
            onCommit={setShortcut}
          />
          <SettingRow
            title={<Trans>Show live caption over orb</Trans>}
            description={
              <Trans>
                Show the last few recognized words in a small caption above
                the orb while you dictate. It fades out when you pause.
              </Trans>
            }
            checked={dictation_caption}
            onChange={setCaption}
          />
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Output</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <OutputModeGroup value={outputMode} onChange={setOutputMode} />
          {outputMode === "batch" ? (
            <SettingRow
              title={<Trans>Paste at cursor</Trans>}
              description={
                <Trans>
                  Paste the transcript into the focused app when you stop. When
                  off it is only copied to the clipboard, so you paste it
                  yourself.
                </Trans>
              }
              checked={dictation_paste_at_cursor}
              onChange={setPasteAtCursor}
            />
          ) : null}
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Cleanup</Trans>
        </h2>
        <CleanupGroup
          value={normalizeCleanupMode(dictation_cleanup)}
          onChange={setCleanup}
        />
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Orb style</Trans>
        </h2>
        <OrbVariantGroup
          value={normalizeOrbVariant(dictation_orb_variant)}
          onChange={setOrbVariant}
        />
      </section>

      <DictationHistorySection />

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Model</Trans>
        </h2>
        <p className="text-muted-foreground text-xs">
          {modelReady ? (
            <Trans>
              Dictation uses your current local transcription model:{" "}
              <span className="text-foreground font-medium">{conn?.model}</span>
              . Change it in the Transcription settings.
            </Trans>
          ) : (
            <Trans>
              Dictation needs a local transcription model. Select and download
              one in the Transcription settings first.
            </Trans>
          )}
        </p>
      </section>
    </div>
  );
}

function RadioCardGroup<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (next: T) => void;
  options: readonly {
    value: T;
    title: ReactNode;
    description: ReactNode;
    preview?: ReactNode;
  }[];
}) {
  const groupName = useId();

  return (
    <div role="radiogroup" className="flex flex-col gap-2">
      {options.map((option) => (
        <label
          key={option.value}
          className={cn([
            "flex cursor-pointer items-start gap-3 rounded-lg border p-3",
            "transition-colors duration-(--motion-duration-state)",
            value === option.value
              ? "border-primary/60 bg-accent/40"
              : "border-border hover:bg-accent/20",
          ])}
        >
          <input
            type="radio"
            name={groupName}
            value={option.value}
            checked={value === option.value}
            onChange={() => onChange(option.value)}
            className="accent-primary mt-0.5 shrink-0"
          />
          <span className="flex flex-1 flex-col gap-1">
            <span className="text-sm font-medium">{option.title}</span>
            <span className="text-muted-foreground text-xs">
              {option.description}
            </span>
          </span>
          {option.preview ? (
            <span className="shrink-0 self-center" aria-hidden>
              {option.preview}
            </span>
          ) : null}
        </label>
      ))}
    </div>
  );
}

/**
 * Where recognized speech goes. `type` = segments are typed into the focused
 * app as they arrive; `batch` = nothing is typed while dictating and the
 * cleaned transcript is delivered once on stop (terminal-friendly).
 */
export function OutputModeGroup({
  value,
  onChange,
}: {
  value: "type" | "batch";
  onChange: (next: string) => void;
}) {
  return (
    <RadioCardGroup
      value={value}
      onChange={onChange}
      options={[
        {
          value: "type",
          title: <Trans>Type as you speak</Trans>,
          description: (
            <Trans>
              Recognized text is typed straight into the focused app while you
              talk.
            </Trans>
          ),
        },
        {
          value: "batch",
          title: <Trans>Collect and deliver when you stop</Trans>,
          description: (
            <Trans>
              Nothing is typed while you talk; stopping cleans up the
              transcript and copies it. Best for terminals.
            </Trans>
          ),
        },
      ]}
    />
  );
}

/** How the finished transcript is cleaned before delivery/history. */
export function CleanupGroup({
  value,
  onChange,
}: {
  value: "none" | "basic" | "llm";
  onChange: (next: string) => void;
}) {
  return (
    <RadioCardGroup
      value={value}
      onChange={onChange}
      options={[
        {
          value: "none",
          title: <Trans>None</Trans>,
          description: (
            <Trans>Keep the transcript exactly as recognized.</Trans>
          ),
        },
        {
          value: "basic",
          title: <Trans>Basic</Trans>,
          description: (
            <Trans>
              Tidy whitespace, capitalize sentences and drop trailing
              fragments. Instant and fully offline.
            </Trans>
          ),
        },
        {
          value: "llm",
          title: <Trans>AI cleanup</Trans>,
          description: (
            <Trans>
              Fix punctuation and remove fillers and false starts with your
              configured AI model. Falls back to basic cleanup when no model
              is available.
            </Trans>
          ),
        },
      ]}
    />
  );
}

/**
 * Base preview size: cobalt-scale variants render at 64px, the particle
 * sphere at its usual 1.5x (96px) - `DictationOrb` applies the per-variant
 * scale exactly like the real orb window does.
 */
const ORB_PREVIEW_BASE_SIZE = 64;
/** Fixed preview slot height so every card row lines up (fits 1.5x = 96px). */
const ORB_PREVIEW_SLOT_PX = 96;

/**
 * Orb look picker: a grid of live preview cards driven by
 * `ORB_VARIANT_REGISTRY`, so new variants show up automatically. The
 * selected card - and any hovered card - runs its orb in the listening
 * phase with a gentle synthetic amplitude loop; the rest stay idle.
 */
export function OrbVariantGroup({
  value,
  onChange,
}: {
  value: DictationOrbVariant;
  onChange: (next: string) => void;
}) {
  const groupName = useId();
  const [hovered, setHovered] = useState<DictationOrbVariant | null>(null);

  return (
    <div
      role="radiogroup"
      data-testid="orb-variant-group"
      className="grid grid-cols-2 gap-3 sm:grid-cols-3"
    >
      {ORB_VARIANT_ORDER.map((variant) => {
        const info = ORB_VARIANT_REGISTRY[variant];
        const selected = value === variant;
        const live = selected || hovered === variant;

        return (
          <OrbPreviewCard
            key={variant}
            groupName={groupName}
            variant={variant}
            title={info.title}
            description={info.description}
            selected={selected}
            live={live}
            onSelect={() => onChange(variant)}
            onHoverChange={(hovering) =>
              setHovered((current) =>
                hovering ? variant : current === variant ? null : current,
              )
            }
          />
        );
      })}
    </div>
  );
}

function OrbPreviewCard({
  groupName,
  variant,
  title,
  description,
  selected,
  live,
  onSelect,
  onHoverChange,
}: {
  groupName: string;
  variant: DictationOrbVariant;
  title: ReactNode;
  description: ReactNode;
  selected: boolean;
  live: boolean;
  onSelect: () => void;
  onHoverChange: (hovering: boolean) => void;
}) {
  const amplitude = useSyntheticAmplitude(live);

  return (
    <label
      data-testid={`orb-preview-card-${variant}`}
      data-selected={selected || undefined}
      onMouseEnter={() => onHoverChange(true)}
      onMouseLeave={() => onHoverChange(false)}
      className={cn([
        // `relative` scopes the `sr-only` radio's absolute positioning to this
        // card. Without it the visually-hidden input anchors to a distant
        // positioned ancestor, so selecting a variant focuses an off-screen
        // element and the browser scrolls the settings page up to reach it.
        "relative flex cursor-pointer flex-col items-center gap-2 rounded-lg border p-3 pt-4 text-center",
        "transition-colors duration-(--motion-duration-state)",
        "focus-within:ring-ring focus-within:ring-2",
        selected
          ? "border-primary/60 ring-primary/50 bg-accent/40 ring-1"
          : "border-border hover:bg-accent/20",
      ])}
    >
      <input
        type="radio"
        name={groupName}
        value={variant}
        checked={selected}
        onChange={onSelect}
        className="sr-only"
      />
      <span
        aria-hidden
        className="flex items-center justify-center"
        style={{ height: ORB_PREVIEW_SLOT_PX }}
      >
        <DictationOrb
          phase={live ? "listening" : "idle"}
          amplitude={amplitude}
          size={ORB_PREVIEW_BASE_SIZE}
          variant={variant}
        />
      </span>
      <span className="flex flex-col gap-0.5">
        <span className="text-sm font-medium">{title}</span>
        <span className="text-muted-foreground text-xs">{description}</span>
      </span>
    </label>
  );
}

/**
 * Gentle looping fake voice level for the live previews: two slow sines sum
 * to something breath-like in ~[0.15, 0.8]. Returns 0 when inactive; a
 * steady mid level under `prefers-reduced-motion` (no animation loop).
 */
function useSyntheticAmplitude(active: boolean): number {
  const [amplitude, setAmplitude] = useState(0);

  useEffect(() => {
    if (!active) {
      setAmplitude(0);
      return;
    }

    if (
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      setAmplitude(0.5);
      return;
    }

    let raf = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const t = (now - start) / 1000;
      const level =
        0.45 + 0.22 * Math.sin(t * 2.3) + 0.16 * Math.sin(t * 3.9 + 1.3);
      setAmplitude(Math.min(Math.max(level, 0), 1));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
    };
  }, [active]);

  return amplitude;
}

/**
 * Dictation history - the "in-app clipboard". Click an entry to copy it
 * again; entries are capped at 50 and pruned automatically.
 */
function DictationHistorySection() {
  const { t } = useLingui();
  const entries = useDictationHistory();

  const handleCopy = async (entry: DictationHistoryEntry) => {
    try {
      await writeText(entry.text);
    } catch {
      // Fall back to the browser clipboard when the plugin is unavailable.
      await navigator.clipboard.writeText(entry.text);
    }
    sonnerToast.success(t`Copied to clipboard`);
  };

  return (
    <section>
      <div className="mb-4 flex items-center justify-between gap-4">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>History</Trans>
        </h2>
        {entries.length > 0 ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void clearDictationHistory()}
          >
            <Trans>Clear all</Trans>
          </Button>
        ) : null}
      </div>
      <DictationHistoryList
        entries={entries}
        onCopy={(entry) => void handleCopy(entry)}
        onDelete={(entry) => void deleteDictationHistoryEntry(entry.id)}
      />
    </section>
  );
}

export function DictationHistoryList({
  entries,
  onCopy,
  onDelete,
}: {
  entries: DictationHistoryEntry[];
  onCopy: (entry: DictationHistoryEntry) => void;
  onDelete: (entry: DictationHistoryEntry) => void;
}) {
  const { t } = useLingui();

  if (entries.length === 0) {
    return (
      <p className="text-muted-foreground text-xs">
        <Trans>
          Nothing here yet - finished dictations appear here so you can copy
          them again.
        </Trans>
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-1" data-testid="dictation-history-list">
      {entries.map((entry) => (
        <li
          key={entry.id}
          className={cn([
            "group flex items-center gap-2 rounded-lg border p-2",
            "border-border hover:bg-accent/20",
          ])}
        >
          <button
            type="button"
            onClick={() => onCopy(entry)}
            title={entry.text}
            className="flex min-w-0 flex-1 cursor-pointer flex-col gap-0.5 text-left"
          >
            <span className="truncate text-sm">{entry.text}</span>
            <span className="text-muted-foreground text-xs">
              {formatRelativeTime(entry.createdAt)}
            </span>
          </button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t`Delete history entry`}
            onClick={() => onDelete(entry)}
          >
            <Trash2Icon className="size-4" />
          </Button>
        </li>
      ))}
    </ul>
  );
}

function formatRelativeTime(createdAt: string): string {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return formatDistanceToNow(date, { addSuffix: true });
}

/**
 * macOS-only: paste-at-cursor and type-as-you-speak both synthesize
 * keystrokes via `enigo`'s `CGEvent` path (`plugins/dictation/src/inject.rs`),
 * which needs the Accessibility permission - without it, injection silently
 * fails. Shown inline whenever the orb is on and the permission has not been
 * granted yet, so the requirement surfaces before a silent no-op paste does.
 * The same permission also has a full row in Settings > Permissions; this is
 * a cheaper, contextual nudge right where dictation is turned on.
 */
export function MacosAccessibilityHint({
  status,
  isPending,
  onRequest,
  onOpen,
}: {
  status: PermissionStatus | undefined;
  isPending: boolean;
  onRequest: () => void;
  onOpen: () => void;
}) {
  if (status === "authorized") {
    return null;
  }

  const neverRequested = status === undefined || status === "neverRequested";

  return (
    <div className="border-amber-500/40 bg-amber-500/10 flex items-center justify-between gap-3 rounded-lg border p-3">
      <div className="flex items-center gap-2">
        <AlertCircleIcon className="size-4 shrink-0 text-amber-500" />
        <p className="text-xs">
          <Trans>
            Notare needs Accessibility access to type and paste dictated text
            into other apps.
          </Trans>
        </p>
      </div>
      <Button
        variant="outline"
        size="sm"
        disabled={isPending}
        onClick={neverRequested ? onRequest : onOpen}
      >
        {neverRequested ? (
          <Trans>Grant access</Trans>
        ) : (
          <Trans>Open Settings</Trans>
        )}
      </Button>
    </div>
  );
}

function SettingRow({
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
