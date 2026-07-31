import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";

import { Button } from "@hypr/ui/components/ui/button";
import { Textarea } from "@hypr/ui/components/ui/textarea";

import { DictionarySettings } from "./dictionary-settings";

import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue, useStoredSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

export function SettingsPersonalization() {
  // Read the raw stored string (not `useConfigValue`, which flattens this
  // setting to `string[]` and drops mapping objects — see
  // `dictionary-settings.tsx` for why).
  const { value: dictionaryRaw } = useStoredSettingValue(
    "personalization_dictionary_terms",
  );
  const setDictionaryRaw = useSetSettingValue(
    "personalization_dictionary_terms",
  );
  const summaryInstructions = useConfigValue("custom_summary_instructions");
  const setSummaryInstructions = useSetSettingValue(
    "custom_summary_instructions",
  );

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Personalization</Trans>} />
      <SummaryInstructionsSettings
        key={summaryInstructions}
        instructions={summaryInstructions}
        onSave={setSummaryInstructions}
      />
      <DictionarySettings
        raw={dictionaryRaw ?? "[]"}
        onSave={setDictionaryRaw}
      />
    </div>
  );
}

export function SummaryInstructionsSettings({
  instructions,
  onSave,
}: {
  instructions: string;
  onSave: (value: string) => void;
}) {
  const { t } = useLingui();
  const form = useForm({
    defaultValues: { instructions },
    onSubmit: ({ value }) => {
      const nextInstructions = value.instructions.trim();
      onSave(nextInstructions);
      form.reset({ instructions: nextInstructions });
    },
  });

  const resetToDefault = () => {
    onSave("");
    form.reset({ instructions: "" });
  };

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <div>
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Summary instructions</Trans>
        </h2>
        <p className="text-muted-foreground mt-1 text-sm">
          <Trans>
            Applied to every generated or regenerated meeting summary.
          </Trans>
        </p>
      </div>

      <form.Field name="instructions">
        {(field) => (
          <Textarea
            aria-label={t`Summary instructions`}
            className="bg-card min-h-40 resize-y rounded-2xl px-4 py-3 text-sm"
            maxLength={4000}
            placeholder={t`Example: Start with a two-sentence overview, then list decisions and action items with owners. Do not use headings.`}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
            onBlur={field.handleBlur}
          />
        )}
      </form.Field>

      <p className="text-muted-foreground text-sm">
        <Trans>
          These instructions take priority over the selected template when they
          conflict. Clear them to use templates as written.
        </Trans>
      </p>

      <div className="flex items-center justify-end gap-2">
        <form.Subscribe selector={(state) => state.values.instructions}>
          {(value) => (
            <Button
              type="button"
              variant="ghost"
              disabled={value.length === 0 && instructions.length === 0}
              onClick={resetToDefault}
            >
              <Trans>Reset to default</Trans>
            </Button>
          )}
        </form.Subscribe>
        <form.Subscribe
          selector={(state) => [state.canSubmit, state.isDirty] as const}
        >
          {([canSubmit, isDirty]) => (
            <Button type="submit" disabled={!canSubmit || !isDirty}>
              <Trans>Save</Trans>
            </Button>
          )}
        </form.Subscribe>
      </div>
    </form>
  );
}
