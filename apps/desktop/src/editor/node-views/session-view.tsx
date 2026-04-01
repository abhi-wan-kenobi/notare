import type { NodeViewComponentProps } from "@handlewithcare/react-prosemirror";
import { format } from "date-fns";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, type ReactNode, useCallback } from "react";

import { cn, safeParseDate } from "@hypr/utils";

import { useLinkedItemOpenBehavior } from "~/editor/session/linked-item-open-behavior";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export const sessionNodeSpec: NodeSpec = {
  group: "block",
  content: "text*",
  marks: "",
  selectable: false,
  attrs: {
    sessionId: { default: null },
  },
  parseDOM: [
    {
      tag: 'div[data-type="session"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        return { sessionId: el.getAttribute("data-session-id") };
      },
    },
  ],
  toDOM(node) {
    return [
      "div",
      { "data-type": "session", "data-session-id": node.attrs.sessionId },
      0,
    ];
  },
};

export const SessionNodeView = forwardRef<
  HTMLDivElement,
  NodeViewComponentProps & { children?: ReactNode }
>(function SessionNodeView({ nodeProps, children, ...htmlAttrs }, ref) {
  const { node } = nodeProps;
  const sessionId = node.attrs.sessionId as string;

  const session = main.UI.useRow("sessions", sessionId, main.STORE_ID);
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isRecording =
    liveSessionId === sessionId &&
    (liveStatus === "active" || liveStatus === "finalizing");
  const createdAt = session?.created_at
    ? safeParseDate(session.created_at as string)
    : null;

  const linkedItemOpenBehavior = useLinkedItemOpenBehavior();
  const openCurrent = useTabs((state) => state.openCurrent);
  const openNew = useTabs((state) => state.openNew);

  const handleMouseDown = useCallback(
    (event: React.MouseEvent) => {
      const target =
        event.target instanceof HTMLElement
          ? event.target
          : event.target instanceof Text
            ? event.target.parentElement
            : null;

      if (target?.closest("[data-session-title]")) {
        return;
      }

      event.stopPropagation();
      event.preventDefault();

      const tab = { id: sessionId, type: "sessions" as const };
      if (linkedItemOpenBehavior === "new") {
        openNew(tab);
        return;
      }

      openCurrent(tab);
    },
    [linkedItemOpenBehavior, openCurrent, openNew, sessionId],
  );

  return (
    <div ref={ref} {...htmlAttrs}>
      <div
        onMouseDown={handleMouseDown}
        className={cn(["flex items-center gap-2 py-1", "cursor-pointer"])}
      >
        {isRecording ? (
          <div
            className="flex size-[18px] shrink-0 items-center justify-center"
            contentEditable={false}
          >
            <div className="size-2.5 animate-pulse rounded-full bg-red-500" />
          </div>
        ) : (
          <Checkbox checked />
        )}
        <span
          data-session-title
          className={cn([
            "min-w-0 flex-1 cursor-text truncate text-sm text-neutral-900",
            !isRecording && "line-through opacity-60",
          ])}
        >
          {children}
        </span>
        {createdAt && (
          <span
            className="ml-auto shrink-0 font-mono text-xs text-neutral-400"
            contentEditable={false}
          >
            {format(createdAt, "h:mm a")}
          </span>
        )}
      </div>
    </div>
  );
});

function Checkbox({ checked }: { checked: boolean }) {
  return (
    <div
      contentEditable={false}
      className={cn([
        "flex size-[18px] shrink-0 items-center justify-center rounded",
        "border-[1.5px]",
        checked ? "border-blue-500 bg-blue-500" : "border-neutral-900",
      ])}
    >
      {checked && (
        <svg
          viewBox="0 0 12 12"
          className="size-3 text-white"
          fill="none"
          stroke="currentColor"
          strokeWidth={2.5}
        >
          <path
            d="M2.5 6l2.5 2.5 4.5-5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
    </div>
  );
}
