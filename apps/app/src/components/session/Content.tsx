import { useState } from "react";
import clsx from "clsx";
import type { Session } from "../../client";

interface ContentProps {
  session: Session;
}

export function Content({ session }: ContentProps) {
  const [showRaw, setShowRaw] = useState(false);
  
  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Content view */}
      <div className="flex-1 overflow-y-auto px-5 pb-20 pt-24">
        <div
          className="prose max-w-none"
          dangerouslySetInnerHTML={{
            __html: showRaw ? session.raw_memo_html : (session.enhanced_memo_html || session.raw_memo_html),
          }}
        />
      </div>
      
      {/* Toggle between raw and enhanced view */}
      {session.enhanced_memo_html && (
        <div className="absolute bottom-4 left-1/2 transform -translate-x-1/2 flex gap-2 z-10">
          <button
            onClick={() => setShowRaw(false)}
            className={clsx(
              "rounded-full px-4 py-2 text-sm font-medium",
              !showRaw
                ? "bg-blue-500 text-white"
                : "bg-neutral-100 text-neutral-700 hover:bg-neutral-200"
            )}
          >
            Enhanced
          </button>
          <button
            onClick={() => setShowRaw(true)}
            className={clsx(
              "rounded-full px-4 py-2 text-sm font-medium",
              showRaw
                ? "bg-blue-500 text-white"
                : "bg-neutral-100 text-neutral-700 hover:bg-neutral-200"
            )}
          >
            Raw
          </button>
        </div>
      )}
    </div>
  );
}
