import { Trans, useLingui } from "@lingui/react/macro";
import { useQueries, useQuery } from "@tanstack/react-query";
import { CheckIcon, Loader2Icon } from "lucide-react";
import { useCallback, useState } from "react";

import type { LocalModel } from "@hypr/plugin-local-stt";
import { cn } from "@hypr/utils";

import { OnboardingButton } from "./shared";

import { formatModelSize } from "~/settings/ai/stt/shared";
import { useSetSettingValues } from "~/settings/queries";
import {
  localSttQueries,
  useLocalModelDownload,
} from "~/stt/useLocalSttModel";

export function TranscriptionSection({
  onContinue,
}: {
  onContinue: () => void;
}) {
  const setSettingValues = useSetSettingValues();

  const supportedModels = useQuery(localSttQueries.supportedModels());
  const models = supportedModels.data ?? [];

  const downloadedQueries = useQueries({
    queries: models.map((model) => localSttQueries.isDownloaded(model.key)),
  });

  const [selectedModel, setSelectedModel] = useState<LocalModel | null>(null);

  const firstDownloadedModel =
    models.find((_, i) => downloadedQueries[i]?.data)?.key ?? null;

  const selectModel = useCallback(
    (model: LocalModel) => {
      setSelectedModel(model);
      setSettingValues({
        current_stt_provider: "hyprnote",
        current_stt_model: model,
      });
    },
    [setSettingValues],
  );

  const canContinue = selectedModel !== null || firstDownloadedModel !== null;

  const handleContinue = () => {
    if (!selectedModel && firstDownloadedModel) {
      selectModel(firstDownloadedModel);
    }
    onContinue();
  };

  return (
    <div className="flex flex-col gap-3">
      {models.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          {supportedModels.isLoading ? (
            <Trans>Loading available models…</Trans>
          ) : (
            <Trans>No local transcription models are available.</Trans>
          )}
        </p>
      ) : (
        <div className="flex flex-col gap-2">
          {models.map((model) => (
            <ModelRow
              key={model.key}
              model={model.key}
              displayName={model.display_name}
              sizeBytes={model.size_bytes}
              isSelected={selectedModel === model.key}
              onSelect={selectModel}
            />
          ))}
        </div>
      )}

      {canContinue && (
        <OnboardingButton onClick={handleContinue}>
          <Trans>Continue</Trans>
        </OnboardingButton>
      )}
    </div>
  );
}

function ModelRow({
  model,
  displayName,
  sizeBytes,
  isSelected,
  onSelect,
}: {
  model: LocalModel;
  displayName: string;
  sizeBytes: number | null;
  isSelected: boolean;
  onSelect: (model: LocalModel) => void;
}) {
  const { t } = useLingui();
  const {
    progress,
    errorMessage,
    isDownloaded,
    showProgress,
    handleDownload,
    handleCancel,
  } = useLocalModelDownload(model, onSelect);

  const sizeLabel = formatModelSize(sizeBytes);

  return (
    <div
      className={cn([
        "border-border bg-muted flex items-center gap-3 rounded-lg border px-4 py-3",
        isDownloaded && "hover:border-primary/50 cursor-pointer",
        isSelected && "border-primary",
      ])}
      onClick={isDownloaded ? () => onSelect(model) : undefined}
    >
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="text-foreground truncate text-sm">{displayName}</span>
        {errorMessage ? (
          <span className="truncate text-xs text-red-500">{errorMessage}</span>
        ) : sizeLabel ? (
          <span className="text-muted-foreground font-mono text-xs">
            {sizeLabel}
          </span>
        ) : null}
      </div>

      {isDownloaded ? (
        <span
          className={cn([
            "flex shrink-0 items-center gap-1 text-sm",
            isSelected ? "text-green-600" : "text-muted-foreground",
          ])}
        >
          <CheckIcon className="size-4" strokeWidth={2.5} />
          {isSelected ? <Trans>Selected</Trans> : <Trans>Downloaded</Trans>}
        </span>
      ) : showProgress ? (
        <div className="flex shrink-0 items-center gap-3">
          <span className="text-muted-foreground flex items-center gap-1.5 text-sm">
            <Loader2Icon className="size-3.5 animate-spin" />
            {Math.round(progress)}%
          </span>
          <button
            onClick={(event) => {
              event.stopPropagation();
              handleCancel();
            }}
            className="text-muted-foreground hover:text-foreground shrink-0 text-sm transition-colors"
          >
            {t`Cancel`}
          </button>
        </div>
      ) : (
        <button
          onClick={handleDownload}
          className="bg-primary text-primary-foreground hover:bg-primary/90 shrink-0 rounded-full px-3 py-1 text-sm font-medium duration-150 hover:scale-[1.01] active:scale-[0.99]"
        >
          {errorMessage ? t`Retry` : t`Download`}
        </button>
      )}
    </div>
  );
}
