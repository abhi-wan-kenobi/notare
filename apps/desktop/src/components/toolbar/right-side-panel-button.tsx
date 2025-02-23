import { PanelRightCloseIcon, PanelRightOpenIcon } from "lucide-react";
import { Button } from "@hypr/ui/components/ui/button";
import { useRightPanel } from "@/contexts/right-panel";
import { useLocation } from "@tanstack/react-router";

export function RightSidePanelButton() {
  const { isExpanded, togglePanel } = useRightPanel();
  const { pathname } = useLocation();

  if (!pathname.includes("/note/")) {
    return null;
  }

  const Icon = isExpanded ? PanelRightCloseIcon : PanelRightOpenIcon;

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={togglePanel}
      className="h-8 w-8"
    >
      <Icon className="size-4" />
    </Button>
  );
}
