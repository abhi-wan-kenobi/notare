import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { EarIcon, EarOffIcon, Loader2Icon, MicIcon, MicOffIcon, Volume2Icon, VolumeOffIcon } from "lucide-react";
import { useEffect, useState } from "react";

import SoundIndicator from "@/components/sound-indicator";
import { useOngoingSession } from "@/contexts";
import { commands as listenerCommands } from "@hypr/plugin-listener";
import { Button } from "@hypr/ui/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@hypr/ui/components/ui/tooltip";

interface ListenButtonProps {
  sessionId: string;
}

export default function ListenButton({ sessionId }: ListenButtonProps) {
  const [open, setOpen] = useState(false);

  const ongoingSessionStore = useOngoingSession((s) => ({
    start: s.start,
    pause: s.pause,
    isCurrent: s.sessionId === sessionId,
    status: s.status,
  }));

  const micMuted = useQuery({
    queryKey: ["mic-muted"],
    queryFn: () => listenerCommands.getMicMuted(),
  });

  const speakerMuted = useQuery({
    queryKey: ["speaker-muted"],
    queryFn: () => listenerCommands.getSpeakerMuted(),
  });

  useEffect(() => {
    micMuted.refetch();
    speakerMuted.refetch();
  }, [ongoingSessionStore.status]);

  const toggleMicMuted = useMutation({
    mutationFn: () => listenerCommands.setMicMuted(!micMuted.data),
    onSuccess: () => micMuted.refetch(),
  });

  const toggleSpeakerMuted = useMutation({
    mutationFn: () => listenerCommands.setSpeakerMuted(!speakerMuted.data),
    onSuccess: () => speakerMuted.refetch(),
  });

  const handleClick = () => {
    if (ongoingSessionStore.status === "inactive") {
      ongoingSessionStore.start(sessionId);
    }
  };

  if (ongoingSessionStore.status === "active" && !ongoingSessionStore.isCurrent) {
    return null;
  }

  if (ongoingSessionStore.status === "loading") {
    return (
      <Button variant="outline" size="icon" onClick={handleClick}>
        <Loader2Icon className="animate-spin" size={20} />
      </Button>
    );
  }

  if (ongoingSessionStore.status === "inactive") {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="outline"
            onClick={handleClick}
            className="p-2"
          >
            <EarOffIcon size={20} />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom" align="end">
          <p>
            <Trans>Start recording</Trans>
          </p>
        </TooltipContent>
      </Tooltip>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <Button
              variant="default"
              onClick={handleClick}
              className="p-2"
            >
              <EarIcon size={20} />
              <SoundIndicator theme="dark" />
            </Button>
          </PopoverTrigger>
        </TooltipTrigger>

        <TooltipContent side="bottom" align="end">
          <p>
            <Trans>Stop recording</Trans>
          </p>
        </TooltipContent>
      </Tooltip>

      <PopoverContent className="w-60" align="end">
        <div className="flex w-full justify-between mb-4">
          <Button variant="ghost" size="icon" onClick={() => toggleMicMuted.mutate()} className="w-full">
            {micMuted.data ? <MicOffIcon className="text-neutral-500" size={20} /> : <MicIcon size={20} />}
            <SoundIndicator theme="light" input="mic" size="long" />
          </Button>

          <Button variant="ghost" size="icon" onClick={() => toggleSpeakerMuted.mutate()} className="w-full">
            {speakerMuted.data ? <VolumeOffIcon className="text-neutral-500" size={20} /> : <Volume2Icon size={20} />}
            <SoundIndicator theme="light" input="speaker" size="long" />
          </Button>
        </div>

        <Button
          variant="destructive"
          onClick={() => {
            ongoingSessionStore.pause();
            setOpen(false);
          }}
          className=" w-full"
        >
          <Trans>Stop listening</Trans>
        </Button>
      </PopoverContent>
    </Popover>
  );
}
