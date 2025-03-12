import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { useSession } from "@/contexts";

import { type Session } from "@hypr/plugin-db";
import { commands as windowsCommands } from "@hypr/plugin-windows";
import { cn } from "@hypr/ui/lib/utils";
import { format } from "date-fns";

export function NoteItem({
  session,
  isActive,
}: {
  session: Session;
  isActive: boolean;
}) {
  const navigate = useNavigate();
  const currentSession = useSession((s) => s.session);
  const sessionDate = new Date(session.created_at);

  const [clicks, setClicks] = useState(0);

  useEffect(() => {
    let singleClickTimer: ReturnType<typeof setTimeout>;
    if (clicks === 1) {
      singleClickTimer = setTimeout(() => {
        handleSingleClick();
        setClicks(0);
      }, 150);
    } else if (clicks === 2) {
      handleDoubleClick();
      setClicks(0);
    }
    return () => clearTimeout(singleClickTimer);
  }, [clicks]);

  const handleClick = () => {
    setClicks((c) => c + 1);
  };

  const handleSingleClick = () => {
    navigate({
      to: "/app/note/$id",
      params: { id: session.id },
    });
  };

  const handleDoubleClick = () => {
    windowsCommands.windowShow({ note: session.id });
  };

  return (
    <button
      onClick={handleClick}
      disabled={isActive}
      className={cn(
        "group flex items-start gap-3 py-2 w-full text-left transition-all rounded px-2",
        isActive ? "bg-neutral-200" : "hover:bg-neutral-100",
      )}
    >
      <div className="flex flex-col items-start gap-1">
        <div className="font-medium text-sm max-w-[180px] truncate">
          {isActive
            ? currentSession?.title || "Untitled"
            : session.title || "Untitled"}
        </div>
        <div className="flex items-center gap-2 text-xs text-neutral-500">
          <span>{format(sessionDate, "M/d/yy")}</span>
        </div>
      </div>
    </button>
  );
}
