import { useCallback, useEffect } from "react";
import { Trans } from "@lingui/react/macro";
import { useLocation, useRouter } from "@tanstack/react-router";
import clsx from "clsx";
import { Button } from "@hypr/ui/components/ui/button";
import { PenIcon } from "lucide-react";

export function NewNoteButton() {
  const { navigate } = useRouter();
  const { pathname } = useLocation();

  const handleClickNewNote = useCallback(() => {
    navigate({ to: "/note/new" });
  }, [navigate]);

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "n") {
        e.preventDefault();
        handleClickNewNote();
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [handleClickNewNote]);

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
