import { ChevronDownIcon } from "lucide-react";
import { useCallback, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useNewNoteAndListen, useNewNoteAndUpload } from "./useNewNote";

import { useNetwork } from "~/contexts/network";
import {
  ActionableTooltipContent,
  RecordingIcon,
  useHasTranscript,
} from "~/session/components/shared";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useSTTConnection } from "~/stt/useSTTConnection";

export function HeaderListenButton() {
  const visible = useHeaderListenVisible();

  if (!visible) {
    return null;
  }

  return <HeaderListenButtonInner />;
}

function useHeaderListenVisible() {
  const currentTab = useTabs((state) => state.currentTab);
  const liveStatus = useListener((state) => state.live.status);
  const loading = useListener((state) => state.live.loading);

  const sessionId = currentTab?.type === "sessions" ? currentTab.id : "";
  const hasTranscript = useHasTranscript(sessionId);

  const isRecording = liveStatus === "active" || liveStatus === "finalizing";

  if (isRecording || loading) return false;
  if (currentTab?.type === "empty") return true;
  if (currentTab?.type === "sessions" && hasTranscript) return true;

  return false;
}

function useHeaderListenState() {
  const { conn: sttConnection, local, isLocalModel } = useSTTConnection();
  const { isOnline } = useNetwork();

  const localServerStatus = local.data?.status ?? "unavailable";
  const isLocalServerLoading = localServerStatus === "loading";
  const isLocalModelNotDownloaded = localServerStatus === "not_downloaded";
  const isOfflineWithCloudModel = !isOnline && !isLocalModel;

  const isDisabled =
    !sttConnection ||
    isLocalServerLoading ||
    isLocalModelNotDownloaded ||
    isOfflineWithCloudModel;

  let warningMessage = "";
  if (isLocalModelNotDownloaded) {
    warningMessage = "Selected model is not downloaded.";
  } else if (isLocalServerLoading) {
    warningMessage = "Local STT server is starting up...";
  } else if (isOfflineWithCloudModel) {
    warningMessage = "You're offline. Use on-device models to continue.";
  } else if (!sttConnection) {
    warningMessage = "Transcription model not available.";
  }

  return { isDisabled, warningMessage };
}

function HeaderListenButtonInner() {
  const { isDisabled, warningMessage } = useHeaderListenState();
  const handleClick = useNewNoteAndListen();
  const handleUpload = useNewNoteAndUpload();
  const openNew = useTabs((state) => state.openNew);
  const [open, setOpen] = useState(false);

  const handleConfigure = useCallback(() => {
    openNew({ type: "ai", state: { tab: "transcription" } });
  }, [openNew]);

  const handleUploadAudio = useCallback(() => {
    setOpen(false);
    handleUpload("audio").catch((error) => {
      console.error("[upload] audio dialog failed:", error);
    });
  }, [handleUpload]);

  const handleUploadTranscript = useCallback(() => {
    setOpen(false);
    handleUpload("transcript").catch((error) => {
      console.error("[upload] transcript dialog failed:", error);
    });
  }, [handleUpload]);

  const button = (
    <button
      type="button"
      onClick={handleClick}
      disabled={isDisabled}
      className={cn([
        "inline-flex items-center justify-center rounded-full text-sm font-medium text-white",
        "gap-2",
        "h-8 pr-8 pl-4",
        "border-2 border-stone-600 bg-stone-800",
        "transition-all duration-200 ease-out",
        "hover:bg-stone-700",
        "disabled:pointer-events-none disabled:opacity-50",
      ])}
    >
      <RecordingIcon />
      <span className="whitespace-nowrap">New meeting</span>
    </button>
  );

  const chevron = (
    <button
      type="button"
      className="absolute top-1/2 right-1.5 z-10 -translate-y-1/2 cursor-pointer text-white/70 transition-colors hover:text-white"
      onClick={(e) => {
        e.stopPropagation();
      }}
    >
      <ChevronDownIcon className="size-3.5" />
      <span className="sr-only">More options</span>
    </button>
  );

  const content = (
    <Popover open={open} onOpenChange={setOpen}>
      <div className="relative flex items-center">
        {warningMessage ? (
          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>{button}</TooltipTrigger>
            <TooltipContent side="bottom">
              <ActionableTooltipContent
                message={warningMessage}
                action={{
                  label: "Configure",
                  handleClick: handleConfigure,
                }}
              />
            </TooltipContent>
          </Tooltip>
        ) : (
          button
        )}
        <PopoverTrigger asChild>{chevron}</PopoverTrigger>
      </div>
      <PopoverContent
        side="bottom"
        align="end"
        sideOffset={4}
        className="w-43 rounded-xl p-1.5"
      >
        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadAudio}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );

  return content;
}
