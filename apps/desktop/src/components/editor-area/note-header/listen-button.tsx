import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { EarIcon, EarOffIcon, MicIcon, MicOffIcon, Volume2Icon, VolumeOffIcon } from "lucide-react";
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
  const queryClient = useQueryClient();

  const ongoingSessionStore = useOngoingSession((s) => ({
    start: s.start,
    pause: s.pause,
    isListening: s.listening,
    isCurrent: s.sessionId === sessionId,
  }));

  const handleClick = () => {
    if (!(ongoingSessionStore.isListening && ongoingSessionStore.isCurrent)) {
      ongoingSessionStore.start(sessionId);
    }
  };

  const micMuted = useQuery({
    queryKey: ["mic-muted"],
    queryFn: () => listenerCommands.getMicMuted(),
  });

  const speakerMuted = useQuery({
    queryKey: ["speaker-muted"],
    queryFn: () => listenerCommands.getSpeakerMuted(),
  });

  useEffect(() => {
    queryClient.invalidateQueries({ queryKey: ["mic-muted"] });
    queryClient.invalidateQueries({ queryKey: ["speaker-muted"] });
  }, [ongoingSessionStore.isListening]);

  const toggleMicMuted = useMutation({
    mutationFn: () => listenerCommands.setMicMuted(!micMuted.data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["mic-muted"] });
    },
  });

  const toggleSpeakerMuted = useMutation({
    mutationFn: () => listenerCommands.setSpeakerMuted(!speakerMuted.data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["speaker-muted"] });
    },
  });

  const button = (
    <Button
      variant={ongoingSessionStore.isListening && ongoingSessionStore.isCurrent ? "default" : "outline"}
      onClick={handleClick}
      className="p-2"
    >
      {ongoingSessionStore.isListening && ongoingSessionStore.isCurrent
        ? <EarIcon size={20} />
        : <EarOffIcon size={20} />}
      {ongoingSessionStore.isListening && ongoingSessionStore.isCurrent && <SoundIndicator theme="dark" />}
    </Button>
  );

  if (!(ongoingSessionStore.isListening && ongoingSessionStore.isCurrent)) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>{button}</TooltipTrigger>
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
          <PopoverTrigger asChild>{button}</PopoverTrigger>
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
