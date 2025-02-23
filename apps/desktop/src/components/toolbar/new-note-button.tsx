import { useCallback } from "react";
import { Trans } from "@lingui/react/macro";
import { useLocation, useRouter } from "@tanstack/react-router";
import clsx from "clsx";
import { Button } from "@hypr/ui/components/ui/button";
import { PenIcon } from "lucide-react";
import { useHotkeys } from "react-hotkeys-hook";

export function NewNoteButton() {
  const { navigate } = useRouter();
  const { pathname } = useLocation();

  const handleClickNewNote = useCallback(() => {
    navigate({ to: "/note/new" });
  }, []);

  useHotkeys(
    "mod+n",
    (event) => {
      event.preventDefault();
      handleClickNewNote();
    },
    { enableOnFormTags: true },
  );

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className={clsx(
          "hidden sm:block",
          "text-xs",
          pathname === "/" ? "opacity-100" : "opacity-0",
        )}
        onClick={handleClickNewNote}
      >
        <Trans>New Note</Trans>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="hover:bg-neutral-200 sm:hidden"
        onClick={handleClickNewNote}
        aria-label="New Note"
      >
        <PenIcon className="size-4" />
      </Button>
    </>
  );
}
