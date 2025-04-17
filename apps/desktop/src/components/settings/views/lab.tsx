import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { commands } from "@/types";
import { commands as flagsCommands } from "@hypr/plugin-flags";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";

export default function Lab() {
  const noteChatQuery = useQuery({
    queryKey: ["flags", "ChatRightPanel"],
    queryFn: () => flagsCommands.isEnabled("ChatRightPanel"),
  });

  const noteChatMutation = useMutation({
    mutationFn: async (enabled: boolean) => {
      if (enabled) {
        flagsCommands.enable("ChatRightPanel");
      } else {
        flagsCommands.disable("ChatRightPanel");
      }
    },
    onSuccess: () => {
      noteChatQuery.refetch();
    },
  });

  const handleToggleNoteChat = (enabled: boolean) => {
    noteChatMutation.mutate(enabled);
  };

  return (
    <div>
      <div className="space-y-4">
        <FeatureFlag
          title="Hyprnote Assistant"
          description="Ask our AI assistant about past notes and upcoming events"
          icon={<ChatLogo />}
          enabled={noteChatQuery.data ?? false}
          onToggle={handleToggleNoteChat}
        />

        <div className="flex flex-row rounded-lg border items-center p-4 gap-2 justify-between">
          <div className="flex flex-col">
            <h2 className="text-sm font-medium">Notifications</h2>
            <p className="text-xs text-muted-foreground">
              This is only for testing purposes. It might crash the app.
            </p>
          </div>
          <div className="flex flex-row gap-2">
            <Button variant="outline" onClick={() => commands.notify()}>Notify</Button>
            <Button variant="outline" onClick={() => commands.notify2()}>Notify2</Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function ChatLogo() {
  const [isAnimating, setIsAnimating] = useState(false);

  useEffect(() => {
    const animationInterval = setInterval(() => {
      setIsAnimating(true);
      const timeout = setTimeout(() => {
        setIsAnimating(false);
      }, 1625);
      return () => clearTimeout(timeout);
    }, 4625);

    return () => clearInterval(animationInterval);
  }, []);

  return (
    <div className="relative w-6 aspect-square flex items-center justify-center">
      <img
        src={isAnimating ? "/assets/dynamic.gif" : "/assets/static.png"}
        alt="AI Assistant"
        className="w-full h-full"
      />
    </div>
  );
}

function FeatureFlag({
  title,
  description,
  icon,
  enabled,
  onToggle,
}: {
  title: string;
  description: string;
  icon: React.ReactNode;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <div className="flex flex-col rounded-lg border p-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex size-6 items-center justify-center">
            {icon}
          </div>
          <div>
            <div className="text-sm font-medium">
              <Trans>{title}</Trans>
            </div>
            <div className="text-xs text-muted-foreground">
              <Trans>{description}</Trans>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Switch
            checked={enabled}
            onCheckedChange={onToggle}
            color="gray"
          />
        </div>
      </div>
    </div>
  );
}
