import { Trans, useLingui } from "@lingui/react/macro";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Trash2Icon } from "lucide-react";
import { type ReactNode, useEffect, useId, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@hypr/utils";

import {
  clearDictationHistory,
  deleteDictationHistoryEntry,
  type DictationHistoryEntry,
  useDictationHistory,
} from "~/dictation/history";
import { DictationOrb, normalizeOrbVariant } from "~/dictation/orb";
import { normalizeOutputMode } from "~/dictation/output-mode";
import { normalizeCleanupMode } from "~/dictation/finalize";
import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * Dictation settings (Windows/Linux): the persistent dictation orb that types
 * recognized speech into whichever app has keyboard focus. The nav hides this
 * section on macOS, which keeps its native dictation path.
 */
export function SettingsDictation() {
  const {
    dictation_enabled,
    dictation_shortcut,
    dictation_output_mode,
    dictation_paste_at_cursor,
    dictation_cleanup,
    dictation_orb_variant,
  } = useConfigValues([
    "dictation_enabled",
    "dictation_shortcut",
    "dictation_output_mode",
    "dictation_paste_at_cursor",
    "dictation_cleanup",
    "dictation_orb_variant",
  ] as const);
  const setEnabled = useSetSettingValue("dictation_enabled");
  const setShortcut = useSetSettingValue("dictation_shortcut");
  const setOutputMode = useSetSettingValue("dictation_output_mode");
  const setPasteAtCursor = useSetSettingValue("dictation_paste_at_cursor");
  const setCleanup = useSetSettingValue("dictation_cleanup");
  const setOrbVariant = useSetSettingValue("dictation_orb_variant");

  const outputMode = normalizeOutputMode(dictation_output_mode);

  const { conn, isLocalModel } = useSTTConnection();
  const modelReady = isLocalModel && !!conn;

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
          <ShortcutRow value={dictation_shortcut} onCommit={setShortcut} />
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

/** Orb look, previewed live next to each option. */
export function OrbVariantGroup({
  value,
  onChange,
}: {
  value: "cobalt" | "particles";
  onChange: (next: string) => void;
}) {
  return (
    <RadioCardGroup
      value={value}
      onChange={onChange}
      options={[
        {
          value: "cobalt",
          title: <Trans>Cobalt</Trans>,
          description: <Trans>The minimal glowing orb.</Trans>,
          preview: <DictationOrb phase="idle" size={28} variant="cobalt" />,
        },
        {
          value: "particles",
          title: <Trans>Particles</Trans>,
          description: (
            <Trans>A voice-reactive particle sphere.</Trans>
          ),
          preview: <DictationOrb phase="idle" size={28} variant="particles" />,
        },
      ]}
    />
  );
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

function ShortcutRow({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (next: string) => void;
}) {
  const { t } = useLingui();
  const titleId = useId();
  const descriptionId = useId();
  const [draft, setDraft] = useState(value);

  // Reflect external changes (another window, defaults) into the input.
  useEffect(() => {
    setDraft(value);
  }, [value]);

  const commit = () => {
    const next = draft.trim().toLowerCase();
    if (!next || next === value) {
      setDraft(value);
      return;
    }
    onCommit(next);
  };

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          <Trans>Toggle shortcut</Trans>
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          <Trans>
            Global shortcut that starts or stops dictation, e.g. ctrl+alt+space.
            Combine ctrl, alt, shift and super with a key.
          </Trans>
        </p>
      </div>
      <Input
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        placeholder={t`ctrl+alt+space`}
        className="w-48 font-mono text-xs"
        spellCheck={false}
      />
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
