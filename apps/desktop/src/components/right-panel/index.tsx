import useViewportState from "beautiful-react-hooks/useViewportState";
import { motion } from "motion/react";

import { useRightPanel } from "@/contexts";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { ChatView, WidgetsView } from "./views";

export default function RightPanel() {
  const { isExpanded, currentView, hidePanel } = useRightPanel();
  const { width } = useViewportState();

  const show = getCurrentWebviewWindowLabel() === "main" && isExpanded;

  if (width < 760) {
    return (
      <div className="relative h-full">
        {show && (
          <div
            className="absolute inset-0 bg-black/30 z-40"
            onClick={hidePanel}
          />
        )}

        <motion.div
          initial={{ opacity: show ? 1 : 0 }}
          animate={{ x: show ? 0 : "100%", opacity: show ? 1 : 0 }}
          transition={{ duration: 0.14 }}
          className="right-panel-container absolute right-0 top-0 z-40 h-full w-[380px] overflow-y-auto border-l bg-neutral-50 scrollbar-none shadow-lg flex flex-col"
        >
          {(currentView === "widget") ? <WidgetsView /> : <ChatView />}
        </motion.div>
      </div>
    );
  }

  return (
    <motion.div
      initial={{ opacity: show ? 1 : 0 }}
      animate={{ width: show ? 380 : 0, opacity: show ? 1 : 0 }}
      transition={{ duration: 0.14 }}
      className="right-panel-container h-full overflow-y-auto border-l bg-neutral-50 scrollbar-none flex flex-col"
    >
      {(currentView === "widget") ? <WidgetsView /> : <ChatView />}
    </motion.div>
  );
}
