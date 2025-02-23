import { useCallback } from "react";
import { PanelRightCloseIcon, PanelRightOpenIcon } from "lucide-react";
import { useRouter, useLocation } from "@tanstack/react-router";
import { Button } from "@hypr/ui/components/ui/button";

export function RightSidePanelButton() {
  const { history } = useRouter();
  const { pathname } = useLocation();

  const handleClickBack = useCallback(() => {
    history.back();
  }, [history]);

  const showButton = pathname.startsWith("/note");

  if (!showButton) {
    return null;
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      className="hover:bg-neutral-200"
      aria-label="Side panel"
    >
      <PanelRightOpenIcon className="size-4" />
    </Button>
  );
}
