import { Trans, useLingui } from "@lingui/react/macro";
import { useId } from "react";

import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import { getBaseLanguageDisplayName } from "~/settings/general/language";

/**
 * Curated target-language list for dictation translation (Lane A2). Kept
 * deliberately short - this is a "translate into" picker, not the full
 * spoken-language list (`CORE_TRANSCRIPTION_LANGUAGE_CODES` in
 * `general/language.ts`). English first since it's the most common target,
 * then Hindi (the app's other well-supported transcription language - see
 * the Hinglish handling in `general/language.ts`), then a handful of other
 * widely dictated languages. Labels come from `Intl.DisplayNames` via
 * `getBaseLanguageDisplayName` so they stay consistent with the rest of the
 * app's language pickers instead of hardcoding English names here.
 */
export const DICTATION_TRANSLATION_TARGET_CODES = [
  "en",
  "hi",
  "es",
  "fr",
  "de",
  "zh",
  "ja",
  "pt",
] as const;

export type DictationTranslationTargetCode =
  (typeof DICTATION_TRANSLATION_TARGET_CODES)[number];

export function normalizeTranslationTarget(
  raw: string | undefined,
): DictationTranslationTargetCode {
  return (
    DICTATION_TRANSLATION_TARGET_CODES as readonly string[]
  ).includes(raw ?? "")
    ? (raw as DictationTranslationTargetCode)
    : "en";
}

/**
 * "Translate dictation" toggle + target-language chips, rendered just under
 * the Cleanup group it depends on (visually subordinate via the indented
 * left-border block, matching the description below). Translation always
 * needs AI cleanup's language model to do the actual translating; when that
 * model isn't available the engine (`finalizeDictation`) falls back to
 * normal cleanup rather than failing, so this never hard-disables the
 * toggle - it just explains the fallback via `modelAvailable`.
 */
export function TranslationSettings({
  enabled,
  target,
  modelAvailable,
  onToggle,
  onTargetChange,
}: {
  enabled: boolean;
  target: string;
  /**
   * Whether AI cleanup currently resolves to a usable language model (cleanup
   * mode is "llm" and a provider/model is connected). When false, translation
   * still works as a setting but will fall back to normal cleanup at
   * dictation time - the helper text below explains that instead of blocking
   * the toggle.
   */
  modelAvailable: boolean;
  onToggle: (checked: boolean) => void;
  onTargetChange: (code: string) => void;
}) {
  const titleId = useId();
  const descriptionId = useId();
  const normalizedTarget = normalizeTranslationTarget(target);
  const targetLabel = getBaseLanguageDisplayName(normalizedTarget);

  return (
    <div className="border-border/70 ml-1 flex flex-col gap-3 border-l-2 pl-4">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 id={titleId} className="mb-1 text-sm font-medium">
            <Trans>Translate dictation</Trans>
          </h3>
          <p id={descriptionId} className="text-muted-foreground text-xs">
            <Trans>
              Dictate in any language, insert the text in {targetLabel}.
              Requires AI cleanup's language model; falls back to normal
              cleanup when no model is available.
            </Trans>
          </p>
        </div>
        <Switch
          checked={enabled}
          onCheckedChange={onToggle}
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
        />
      </div>

      {!modelAvailable ? (
        <p className="text-muted-foreground text-xs">
          <Trans>
            AI cleanup isn't using a language model right now, so translation
            will fall back to normal cleanup. Switch cleanup to "AI cleanup"
            and configure a model in Settings → Intelligence to translate.
          </Trans>
        </p>
      ) : null}

      {enabled ? (
        <TranslationTargetGroup
          value={normalizedTarget}
          onChange={onTargetChange}
        />
      ) : null}
    </div>
  );
}

function TranslationTargetGroup({
  value,
  onChange,
}: {
  value: DictationTranslationTargetCode;
  onChange: (code: string) => void;
}) {
  const { t } = useLingui();
  const groupName = useId();

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-muted-foreground text-xs">
        <Trans>Translate to</Trans>
      </span>
      <div
        role="radiogroup"
        aria-label={t`Translation target language`}
        className="flex flex-wrap gap-1.5"
      >
        {DICTATION_TRANSLATION_TARGET_CODES.map((code) => {
          const selected = value === code;
          return (
            <label
              key={code}
              className={cn([
                "cursor-pointer rounded-full border px-2.5 py-1 text-xs",
                "transition-colors duration-(--motion-duration-state)",
                selected
                  ? "border-primary/60 bg-accent/40 text-foreground font-medium"
                  : "border-border text-muted-foreground hover:bg-accent/20",
              ])}
            >
              <input
                type="radio"
                name={groupName}
                value={code}
                checked={selected}
                onChange={() => onChange(code)}
                className="sr-only"
              />
              {getBaseLanguageDisplayName(code)}
            </label>
          );
        })}
      </div>
    </div>
  );
}
