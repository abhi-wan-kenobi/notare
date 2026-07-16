import { Trans, useLingui } from "@lingui/react/macro";
import { type ReactNode, useEffect, useId, useState } from "react";

import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

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
  const { dictation_enabled, dictation_shortcut, dictation_output_mode } =
    useConfigValues([
      "dictation_enabled",
      "dictation_shortcut",
      "dictation_output_mode",
    ] as const);
  const setEnabled = useSetSettingValue("dictation_enabled");
  const setShortcut = useSetSettingValue("dictation_shortcut");
  const setOutputMode = useSetSettingValue("dictation_output_mode");

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
        <OutputModeGroup value={dictation_output_mode} onChange={setOutputMode} />
      </section>

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

/**
 * Where recognized speech goes. `type` = segments are typed into the focused
 * app as they arrive; `batch-paste` = nothing is typed while dictating and
 * the cleaned transcript is pasted once on stop (terminal-friendly).
 */
function OutputModeGroup({
  value,
  onChange,
}: {
  value: string;
  onChange: (next: string) => void;
}) {
  const groupName = useId();
  const selected = value === "batch-paste" ? "batch-paste" : "type";

  const options = [
    {
      mode: "type",
      title: <Trans>Type as you speak</Trans>,
      description: (
        <Trans>
          Recognized text is typed straight into the focused app while you
          talk.
        </Trans>
      ),
    },
    {
      mode: "batch-paste",
      title: <Trans>Paste when you stop</Trans>,
      description: (
        <Trans>
          Nothing is typed while you talk; stopping cleans up the transcript,
          copies it, and pastes it once. Best for terminals.
        </Trans>
      ),
    },
  ] as const;

  return (
    <div role="radiogroup" className="flex flex-col gap-2">
      {options.map((option) => (
        <label
          key={option.mode}
          className={cn([
            "flex cursor-pointer items-start gap-3 rounded-lg border p-3",
            "transition-colors duration-(--motion-duration-state)",
            selected === option.mode
              ? "border-primary/60 bg-accent/40"
              : "border-border hover:bg-accent/20",
          ])}
        >
          <input
            type="radio"
            name={groupName}
            value={option.mode}
            checked={selected === option.mode}
            onChange={() => onChange(option.mode)}
            className="accent-primary mt-0.5 shrink-0"
          />
          <span className="flex flex-col gap-1">
            <span className="text-sm font-medium">{option.title}</span>
            <span className="text-muted-foreground text-xs">
              {option.description}
            </span>
          </span>
        </label>
      ))}
    </div>
  );
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
