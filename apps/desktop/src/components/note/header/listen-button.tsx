import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckIcon, EarIcon, EarOffIcon, XIcon } from "lucide-react";
import { useEffect, useState } from "react";

import SoundIndicator from "@/components/sound-indicator";

import { commands as listenerCommands } from "@hypr/plugin-listener";
import { Button } from "@hypr/ui/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@hypr/ui/components/ui/tooltip";

interface ListenButtonProps {
  isListening: boolean;
  onClick: () => void;
  onStop?: () => void;
  isCurrent: boolean;
}

export default function ListenButton({
  isListening,
  onClick,
  onStop,
  isCurrent,
}: ListenButtonProps) {
  const [open, setOpen] = useState(false);
  const queryClient = useQueryClient();

  const handleClick = () => {
    if (!(isListening && isCurrent)) {
      onClick();
    }
  };

  useEffect(() => {
    queryClient.invalidateQueries({ queryKey: ["mic-muted"] });
    queryClient.invalidateQueries({ queryKey: ["speaker-muted"] });
  }, [isListening]);

  const micMuted = useQuery({
    queryKey: ["mic-muted"],
    queryFn: () => listenerCommands.getMicMuted(),
  });

  const speakerMuted = useQuery({
    queryKey: ["speaker-muted"],
    queryFn: () => listenerCommands.getSpeakerMuted(),
  });

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
      variant={isListening && isCurrent ? "default" : "outline"}
      onClick={handleClick}
      className="p-2"
    >
      {isListening && isCurrent ? <EarIcon size={20} /> : <EarOffIcon size={20} />}
      {isListening && isCurrent && <SoundIndicator theme="dark" />}
    </Button>
  );

  if (!(isListening && isCurrent)) {
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

      <PopoverContent className="w-60 p-4" align="end">
        <Button
          variant="outline"
          onClick={() => toggleMicMuted.mutate()}
        >
          <Trans>Mic</Trans>
          {micMuted.data ? <XIcon size={16} /> : <CheckIcon size={16} />}
        </Button>

        <Button
          variant="outline"
          onClick={() => toggleSpeakerMuted.mutate()}
        >
          <Trans>Speaker</Trans>
          {speakerMuted.data ? <XIcon size={16} /> : <CheckIcon size={16} />}
        </Button>

        <div className="flex flex-col items-center gap-3 w-full">
          <div className="text-sm font-medium">
            <Trans>Stop listening to the meeting?</Trans>
          </div>

          <Button
            variant="destructive"
            onClick={() => {
              onStop?.();
              setOpen(false);
            }}
            className=" w-full"
          >
            <Trans>Stop</Trans>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
