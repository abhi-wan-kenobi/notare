import { Trans } from "@lingui/react/macro";
import { useMutation, UseMutationResult, useQuery } from "@tanstack/react-query";
import {
  CheckIcon,
  MicIcon,
  MicOffIcon,
  PauseIcon,
  PlayIcon,
  StopCircleIcon,
  Volume2Icon,
  VolumeOffIcon,
} from "lucide-react";
import { useEffect, useState } from "react";

import SoundIndicator from "@/components/sound-indicator";
import { useHypr } from "@/contexts";
import { useEnhancePendingState } from "@/hooks/enhance-pending";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as listenerCommands } from "@hypr/plugin-listener";
import { commands as localSttCommands } from "@hypr/plugin-local-stt";
import { Button } from "@hypr/ui/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/ui/lib/utils";
import { useOngoingSession, useSession } from "@hypr/utils/contexts";
import ShinyButton from "./shiny-button";

export default function ListenButton({ sessionId }: { sessionId: string }) {
  const { userId, onboardingSessionId } = useHypr();
  const isOnboarding = sessionId === onboardingSessionId;

  const modelDownloaded = useQuery({
    queryKey: ["check-stt-model-downloaded"],
    refetchInterval: 1000,
    queryFn: async () => {
      const currentModel = await localSttCommands.getCurrentModel();
      const isDownloaded = await localSttCommands.isModelDownloaded(currentModel);
      return isDownloaded;
    },
  });

  const ongoingSessionStatus = useOngoingSession((s) => s.status);
  const ongoingSessionId = useOngoingSession((s) => s.sessionId);
  const ongoingSessionStore = useOngoingSession((s) => ({
    start: s.start,
    resume: s.resume,
    pause: s.pause,
    stop: s.stop,
    loading: s.loading,
    hasShownConsent: s.hasShownConsent,
    setHasShownConsent: s.setHasShownConsent,
  }));

  const isEnhancePending = useEnhancePendingState(sessionId);
  const nonEmptySession = useSession(
    sessionId,
    (s) => !!(s.session.words.length > 0 || s.session.enhanced_memo_html),
  );
  const meetingEnded = isEnhancePending || nonEmptySession;

  const handleStartSession = () => {
    if (ongoingSessionStatus === "inactive") {
      ongoingSessionStore.start(sessionId);

      analyticsCommands.event({
        event: "onboarding_video_started",
        distinct_id: userId,
        session_id: sessionId,
      });
    }
  };

  const handleResumeSession = () => {
    ongoingSessionStore.resume();
  };

  if (ongoingSessionStore.loading) {
    return (
      <div className="w-9 h-9 flex items-center justify-center">
        <Spinner color="black" />
      </div>
    );
  }

  if (ongoingSessionStatus === "running_paused" && sessionId === ongoingSessionId) {
    return (
      <button
        disabled={!modelDownloaded.data}
        onClick={handleResumeSession}
        className={cn(
          "w-16 h-9 rounded-full transition-all hover:scale-95 cursor-pointer outline-none p-0 flex items-center justify-center text-xs font-medium",
          "bg-red-100 border-2 border-red-400 text-red-600",
          "shadow-[0_0_0_2px_rgba(255,255,255,0.8)_inset]",
        )}
      >
        <Trans>Resume</Trans>
      </button>
    );
  }

  if (ongoingSessionStatus === "inactive") {
    const buttonProps = {
      disabled: !modelDownloaded.data || (meetingEnded && isEnhancePending),
      onClick: handleStartSession,
    };

    if (!meetingEnded) {
      return isOnboarding
        ? <WhenInactiveAndMeetingNotEndedOnboarding {...buttonProps} />
        : <WhenInactiveAndMeetingNotEnded {...buttonProps} />;
    } else {
      return isOnboarding
        ? <WhenInactiveAndMeetingEndedOnboarding {...buttonProps} />
        : <WhenInactiveAndMeetingEnded {...buttonProps} />;
    }
  }

  if (ongoingSessionStatus === "running_active") {
    if (sessionId !== ongoingSessionId) {
      return null;
    }

    return <WhenActive />;
  }
}

function WhenInactiveAndMeetingNotEnded({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          disabled={disabled}
          onClick={onClick}
          className={cn([
            "w-9 h-9 rounded-full border-2 transition-all hover:scale-95 cursor-pointer outline-none p-0 flex items-center justify-center shadow-[inset_0_0_0_2px_rgba(255,255,255,0.8)]",
            disabled ? "bg-neutral-200 border-neutral-400" : "bg-red-500 border-neutral-400",
          ])}
        >
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" align="end">
        <p>
          <Trans>Start recording</Trans>
        </p>
      </TooltipContent>
    </Tooltip>
  );
}

function WhenInactiveAndMeetingEnded({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <button
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={cn(
        "w-16 h-9 rounded-full transition-all outline-none p-0 flex items-center justify-center text-xs font-medium",
        "bg-neutral-200 border-2 border-neutral-400 text-neutral-600",
        "shadow-[0_0_0_2px_rgba(255,255,255,0.8)_inset]",
        !disabled
          ? "hover:opacity-100 hover:bg-red-100 hover:text-red-600 hover:border-red-400 hover:scale-95 cursor-pointer"
          : "opacity-10 cursor-progress",
      )}
    >
      <Trans>{disabled ? "Wait..." : isHovered ? "Resume" : "Ended"}</Trans>
    </button>
  );
}

function WhenInactiveAndMeetingNotEndedOnboarding({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  return (
    <ShinyButton
      disabled={disabled}
      onClick={onClick}
      className={cn([
        "w-24 h-9 rounded-full border-2 transition-all cursor-pointer outline-none p-0 flex items-center justify-center gap-1",
        "bg-neutral-800 border-neutral-700 text-white text-xs font-medium",
      ])}
      style={{
        boxShadow: "0 0 0 2px rgba(255, 255, 255, 0.8) inset",
      }}
    >
      <PlayIcon size={14} />
      <Trans>Play video</Trans>
    </ShinyButton>
  );
}

function WhenInactiveAndMeetingEndedOnboarding({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  return (
    <button
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "w-28 h-9 rounded-full outline-none p-0 flex items-center justify-center gap-1 text-xs font-medium",
        "bg-neutral-200 border-2 border-neutral-400 text-neutral-600",
        "shadow-[0_0_0_2px_rgba(255,255,255,0.8)_inset]",
        !disabled
          ? "hover:bg-neutral-300 hover:text-neutral-800 hover:border-neutral-500 transition-all hover:scale-95 cursor-pointer"
          : "opacity-10 cursor-progress",
      )}
    >
      <PlayIcon size={14} />
      <Trans>{disabled ? "Wait..." : "Play again"}</Trans>
    </button>
  );
}

export function WhenActive() {
  const [open, setOpen] = useState(true);
  const { sessionId } = useOngoingSession((s) => ({ sessionId: s.sessionId }));
  const { onboardingSessionId } = useHypr();
  const isOnboarding = sessionId === onboardingSessionId;

  const ongoingSessionStore = useOngoingSession((s) => ({
    pause: s.pause,
    stop: s.stop,
    loading: s.loading,
  }));

  const { showConsent, setHasShownConsent } = useConsentManagement(isOnboarding);
  const audioControls = useAudioControls();

  useEffect(() => {
    if (showConsent) {
      listenerCommands.setSpeakerMuted(true).then(() => {
        audioControls.refetchSpeakerMuted();
      });
    }
  }, [showConsent, audioControls.refetchSpeakerMuted]);

  const handleConsentDecision = useMutation({
    mutationFn: async (recordEveryone: boolean) => {
      if (recordEveryone) {
        await listenerCommands.setSpeakerMuted(false);
      } else {
        await listenerCommands.setSpeakerMuted(true);
        await listenerCommands.setMicMuted(false);
      }
      setHasShownConsent(true);
    },
    onSuccess: () => {
      audioControls.refetchSpeakerMuted();
      audioControls.refetchMicMuted();
    },
  });

  const handlePauseSession = () => {
    ongoingSessionStore.pause();
    setOpen(false);
  };

  const handleStopSession = () => {
    ongoingSessionStore.stop();
    setOpen(false);
  };

  const handleOpenChange = (isOpen: boolean) => {
    if (!isOpen && !showConsent) {
      setOpen(false);
    } else if (showConsent) {
      setOpen(true);
    } else {
      setOpen(isOpen);
    }
  };

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          className={cn([
            open && "hover:scale-95",
            "w-14 h-9 rounded-full bg-red-100 border-2 transition-all border-red-400 cursor-pointer outline-none p-0 flex items-center justify-center",
            "shadow-[0_0_0_2px_rgba(255,255,255,0.8)_inset]",
          ])}
        >
          <SoundIndicator color="#ef4444" size="long" />
        </button>
      </PopoverTrigger>

      <PopoverContent className="w-64" align="end">
        {showConsent ? <ConsentDialog onDecision={handleConsentDecision} /> : (
          <RecordingControls
            audioControls={audioControls}
            onPause={handlePauseSession}
            onStop={handleStopSession}
          />
        )}
      </PopoverContent>
    </Popover>
  );
}

function ConsentDialog({
  onDecision,
}: {
  onDecision: UseMutationResult<void, Error, boolean, unknown>;
}) {
  return (
    <div className="flex flex-col gap-4">
      <div>
        <h4 className="font-medium text-sm mb-2">
          <Trans>Recording Started</Trans>
        </h4>
        <p className="text-xs text-gray-500 mb-4">
          <Trans>Did you get consent from everyone in the meeting?</Trans>
        </p>
        <div className="flex flex-col gap-2">
          <Button
            variant="default"
            onClick={() => onDecision.mutate(true)}
            className="w-full"
          >
            <CheckIcon size={16} className="mr-1" />
            <Trans>Yes, activate speaker</Trans>
          </Button>
          <Button
            variant="outline"
            onClick={() => onDecision.mutate(false)}
            className="w-full"
          >
            <MicIcon size={16} className="mr-1" />
            <Trans>Record me only</Trans>
          </Button>
        </div>
      </div>
    </div>
  );
}

function RecordingControls({
  audioControls,
  onPause,
  onStop,
}: {
  audioControls: ReturnType<typeof useAudioControls>;
  onPause: () => void;
  onStop: () => void;
}) {
  return (
    <>
      <div className="flex w-full justify-between mb-4">
        <AudioControlButton
          isMuted={audioControls.isMicMuted}
          onClick={() => audioControls.toggleMicMuted.mutate()}
          type="mic"
        />
        <AudioControlButton
          isMuted={audioControls.isSpeakerMuted}
          onClick={() => audioControls.toggleSpeakerMuted.mutate()}
          type="speaker"
        />
      </div>

      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={onPause}
          className="w-full"
        >
          <PauseIcon size={16} />
          <Trans>Pause</Trans>
        </Button>
        <Button
          variant="destructive"
          onClick={onStop}
          className="w-full"
        >
          <StopCircleIcon size={16} />
          <Trans>Stop</Trans>
        </Button>
      </div>
    </>
  );
}

function AudioControlButton({
  type,
  isMuted,
  onClick,
  disabled,
}: {
  type: "mic" | "speaker";
  isMuted?: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  const Icon = type === "mic"
    ? isMuted
      ? MicOffIcon
      : MicIcon
    : isMuted
    ? VolumeOffIcon
    : Volume2Icon;

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={onClick}
      className="w-full"
      disabled={disabled}
    >
      <Icon className={cn(isMuted ? "text-neutral-500" : "", disabled && "text-neutral-300")} size={20} />
      {!disabled && <SoundIndicator input={type} size="long" />}
    </Button>
  );
}

function useConsentManagement(isOnboarding: boolean) {
  const ongoingSessionStore = useOngoingSession((s) => ({
    hasShownConsent: s.hasShownConsent,
    setHasShownConsent: s.setHasShownConsent,
  }));

  const effectiveHasShownConsent = isOnboarding ? true : ongoingSessionStore.hasShownConsent;
  const showConsent = !effectiveHasShownConsent && !isOnboarding;

  return {
    showConsent,
    setHasShownConsent: ongoingSessionStore.setHasShownConsent,
  };
}

function useAudioControls() {
  const { data: isMicMuted, refetch: refetchMicMuted } = useQuery({
    queryKey: ["mic-muted"],
    queryFn: () => listenerCommands.getMicMuted(),
  });

  const { data: isSpeakerMuted, refetch: refetchSpeakerMuted } = useQuery({
    queryKey: ["speaker-muted"],
    queryFn: () => listenerCommands.getSpeakerMuted(),
  });

  const toggleMicMuted = useMutation({
    mutationFn: () => listenerCommands.setMicMuted(!isMicMuted),
    onSuccess: () => refetchMicMuted(),
  });

  const toggleSpeakerMuted = useMutation({
    mutationFn: () => listenerCommands.setSpeakerMuted(!isSpeakerMuted),
    onSuccess: () => refetchSpeakerMuted(),
  });

  return {
    isMicMuted,
    isSpeakerMuted,
    toggleMicMuted,
    toggleSpeakerMuted,
    refetchSpeakerMuted,
    refetchMicMuted,
  };
}
