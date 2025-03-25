import { useNavigate } from "@tanstack/react-router";
import { motion } from "motion/react";

import { useHyprSearch, useSession } from "@/contexts";
import { cn } from "@hypr/ui/lib/utils";
import { format } from "@hypr/utils/datetime";

export default function OngoingSession({
  sessionId,
}: {
  sessionId: string;
}) {
  const navigate = useNavigate();
  const session = useSession(sessionId, (s) => s.session);

  const { setQuery } = useHyprSearch((s) => ({
    setQuery: s.setQuery,
  }));

  const handleClick = () => {
    setQuery("");

    navigate({
      to: "/app/note/$id",
      params: { id: sessionId },
    });
  };

  const html2text = (html: string) => {
    return html.replace(/<[^>]*>?/g, "");
  };

  return (
    <motion.div
      key={sessionId}
      layout
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.2 }}
      className="p-2 mb-2"
    >
      <button
        onClick={handleClick}
        className={cn(
          "group flex items-start gap-3 w-full text-left transition-all",
          "bg-neutral-600 hover:bg-neutral-700 p-2 rounded-lg",
        )}
      >
        <div className="flex flex-col items-start gap-1 max-w-[180px] truncate">
          <div className="flex items-center justify-between gap-1">
            <h3 className="font-medium text-sm text-neutral-300">
              {session.title || "Untitled"}
            </h3>
          </div>

          <div className="flex items-center gap-3 text-xs text-neutral-400">
            <span className="font-medium">{format(session.created_at, "h:mm a")}</span>
            <span className="text-xs">
              {html2text(session.enhanced_memo_html || session.raw_memo_html)}
            </span>
          </div>
        </div>
      </button>
    </motion.div>
  );
}
