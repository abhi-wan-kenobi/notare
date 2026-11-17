import { Trans } from "@lingui/react/macro";

import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import {
  SelectBatchModel,
  SelectLiveModel,
  TranscriptionLanguageWarningToast,
} from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function STT() {
  return (
    <SttSettingsProvider>
      <TranscriptionLanguageWarningToast />
      <div className="flex flex-col gap-6">
        <SettingsPageTitle title={<Trans>Transcription</Trans>} />
        <SelectLiveModel />
        <SelectBatchModel />
        <section className="border-border rounded-lg border px-4 py-3 text-sm">
          <h3 className="font-medium">
            <Trans>Hindi and Hinglish on Mac</Trans>
          </h3>
          <p className="text-muted-foreground mt-2 leading-relaxed">
            <Trans>
              Choose Hindi or Hinglish in Spoken languages, and use a
              multilingual Whisper model. Parakeet and English-only models do
              not cover Hindi.
            </Trans>
          </p>
          <p className="text-muted-foreground mt-2 leading-relaxed">
            <Trans>
              For live notes, try Whisper Small or Large Turbo. For a
              quality-first final pass, download Whisper Large v3 (Multilingual,
              Q5), about 1 GB. It runs locally with Metal on Apple Silicon, but
              uses more memory and takes longer than Turbo. Keep the recording
              for the final pass.
            </Trans>
          </p>
          <p className="text-muted-foreground mt-2 leading-relaxed">
            <Trans>
              Hinglish lets Whisper detect Hindi and English per audio chunk.
              Hindi text may be written in Devanagari; Romanized Hindi is not
              guaranteed.
            </Trans>
          </p>
        </section>
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}
