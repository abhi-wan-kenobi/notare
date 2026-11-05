import { Trans } from "@lingui/react/macro";
import {
  AudioLinesIcon,
  CalendarPlusIcon,
  FileDownIcon,
  FileTextIcon,
  MoreHorizontalIcon,
  PictureInPicture2Icon,
  ShareIcon,
  SquareArrowOutUpRightIcon,
} from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { DeleteNote } from "./delete";
import { ExportModal } from "./export-modal";
import { FollowupEmail } from "./followup";
import { Listening } from "./listening";
import { ShowInFinder } from "./misc";
import {
  ScheduleFollowupDialog,
  ScheduleFollowupMenuItem,
} from "./schedule-followup";

import { openFloatingMeetingPanel } from "~/meeting-float/host";
import {
  useCurrentNoteHasContent,
  useHasTranscript,
} from "~/session/components/shared";
import { openStandaloneNoteWindow } from "~/session/window";
import { useConfigValue } from "~/shared/config";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function OverflowButton({
  allowListening = true,
  standaloneWindow = false,
  showMeetingActions = false,
  sessionId,
  currentView,
}: {
  allowListening?: boolean;
  standaloneWindow?: boolean;
  showMeetingActions?: boolean;
  sessionId: string;
  currentView: EditorView;
}) {
  const [open, setOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const [isScheduleFollowupOpen, setIsScheduleFollowupOpen] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const currentNoteHasContent = useCurrentNoteHasContent(
    sessionId,
    currentView,
  );
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const isMeetingInProgress =
    sessionMode === "active" || sessionMode === "finalizing";
  const showListeningAction =
    allowListening && (!hasTranscript || isMeetingInProgress);
  const showUploadActions =
    !hasTranscript && !currentNoteHasContent && !isMeetingInProgress;
  const canOpenFloatingPanel =
    allowListening && floatingBarEnabled && sessionMode === "active";
  const hasMeetingActions =
    showListeningAction || showUploadActions || canOpenFloatingPanel;
  const openExportModal = () => {
    setOpen(false);
    requestAnimationFrame(() => setIsExportModalOpen(true));
  };
  const handleUploadAudio = () => {
    setOpen(false);
    uploadAudio();
  };
  const handleUploadTranscript = () => {
    setOpen(false);
    uploadTranscript();
  };
  const handleOpenFloatingPanel = () => {
    setOpen(false);
    void openFloatingMeetingPanel({
      sessionId,
      enabled: floatingBarEnabled,
    });
  };
  const handleOpenStandaloneWindow = () => {
    setOpen(false);
    void openStandaloneNoteWindow(sessionId);
  };

  return (
    <>
      {showMeetingActions ? (
        <>
          <Button
            size="sm"
            variant="outline"
            data-tauri-drag-region="false"
            onClick={openExportModal}
            className="text-foreground bg-card hover:bg-accent border-border/80 h-7 gap-1.5 rounded-md px-2.5 text-xs"
          >
            <ShareIcon size={13} />
            <Trans>Share</Trans>
          </Button>
          <Button
            size="sm"
            variant="ghost"
            data-tauri-drag-region="false"
            onClick={() => setIsScheduleFollowupOpen(true)}
            className="text-muted-foreground hover:text-foreground hover:bg-accent h-7 gap-1.5 rounded-md px-2.5 text-xs"
          >
            <CalendarPlusIcon size={13} />
            <Trans>Schedule follow-up</Trans>
          </Button>
        </>
      ) : null}
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button
            size="icon"
            variant="ghost"
            data-tauri-drag-region="false"
            className="text-muted-foreground hover:bg-accent hover:text-foreground rounded-full"
          >
            <MoreHorizontalIcon size={16} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent variant="app" align="end" className="w-56">
          <AppFloatingPanel className="overflow-hidden p-1">
            <DropdownMenuItem
              onClick={openExportModal}
              className="cursor-pointer"
            >
              <FileDownIcon />
              <span>
                <Trans>Export</Trans>
              </span>
            </DropdownMenuItem>
            <FollowupEmail
              sessionId={sessionId}
              onSelect={() => setOpen(false)}
            />
            <ScheduleFollowupMenuItem
              onSelect={() => {
                setOpen(false);
                setIsScheduleFollowupOpen(true);
              }}
            />
            <DropdownMenuSeparator />
            {showListeningAction && (
              <Listening sessionId={sessionId} hasTranscript={hasTranscript} />
            )}
            {showUploadActions && (
              <>
                <DropdownMenuItem
                  onClick={handleUploadAudio}
                  className="cursor-pointer"
                >
                  <AudioLinesIcon />
                  <span>
                    <Trans>Upload audio</Trans>
                  </span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleUploadTranscript}
                  className="cursor-pointer"
                >
                  <FileTextIcon />
                  <span>
                    <Trans>Upload transcript</Trans>
                  </span>
                </DropdownMenuItem>
              </>
            )}
            {canOpenFloatingPanel && (
              <DropdownMenuItem
                onClick={handleOpenFloatingPanel}
                className="cursor-pointer"
              >
                <PictureInPicture2Icon />
                <span>
                  <Trans>Open floating panel</Trans>
                </span>
              </DropdownMenuItem>
            )}
            {hasMeetingActions && <DropdownMenuSeparator />}
            {!standaloneWindow && (
              <DropdownMenuItem
                onClick={handleOpenStandaloneWindow}
                className="cursor-pointer"
              >
                <SquareArrowOutUpRightIcon />
                <span>
                  <Trans>Open in New Window</Trans>
                </span>
              </DropdownMenuItem>
            )}
            <ShowInFinder sessionId={sessionId} />
            <DeleteNote sessionId={sessionId} />
          </AppFloatingPanel>
        </DropdownMenuContent>
      </DropdownMenu>
      <ExportModal
        sessionId={sessionId}
        currentView={currentView}
        open={isExportModalOpen}
        onOpenChange={setIsExportModalOpen}
      />
      <ScheduleFollowupDialog
        sessionId={sessionId}
        open={isScheduleFollowupOpen}
        onOpenChange={setIsScheduleFollowupOpen}
      />
    </>
  );
}
