import { PanelRightCloseIcon, PanelRightOpenIcon } from "lucide-react";
import { Button } from "@hypr/ui/components/ui/button";
import { useRightPanel } from "@/contexts/right-panel";
import { useLocation } from "@tanstack/react-router";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export function RightSidePanelButton() {
  const { isExpanded, togglePanel } = useRightPanel();
  const { pathname } = useLocation();

  if (!pathname.includes("/note/")) {
    return null;
  }

  const Icon = isExpanded ? PanelRightCloseIcon : PanelRightOpenIcon;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          onClick={togglePanel}
          className="h-8 w-8"
        >
          <Icon className="size-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        <p>Toggle right panel (⌘R)</p>
      </TooltipContent>
    </Tooltip>
  );
}
