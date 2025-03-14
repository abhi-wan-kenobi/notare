import { useQuery } from "@tanstack/react-query";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { ChevronsLeftIcon, MenuIcon } from "lucide-react";

import { useLeftSidebar } from "@/contexts";
import { commands as windowsCommands } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@hypr/ui/components/ui/tooltip";

import Shortcut from "../../shortcut";

export function LeftSidebarButton({ type }: { type: "toolbar" | "sidebar" }) {
  const osType = useQuery({
    queryKey: ["osType"],
    queryFn: () => getOsType(),
    staleTime: Infinity,
  });

  const { isExpanded, togglePanel } = useLeftSidebar();

  const Icon = isExpanded ? ChevronsLeftIcon : MenuIcon;

  if (type === "toolbar" && isExpanded) {
    return null;
  }

  const handleClickCalendar = () => {
    windowsCommands.windowShow("calendar");
  };

  return (
    <div className={osType.data === "macos" ? "pl-[70px]" : ""}>
      <button
        className="rounded-md px-1 bg-neutral-300 hover:bg-neutral-400"
        onClick={handleClickCalendar}
      >
        cal
      </button>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            onClick={togglePanel}
            className="hover:bg-neutral-200"
          >
            <Icon className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <p>
            Toggle left sidebar <Shortcut macDisplay="⌘L" windowsDisplay="Ctrl+L" />
          </p>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
