import { motion } from "motion/react";
import { useRightPanel } from "@/contexts/right-panel";
import { WidgetContainer } from "./widgets/widget-container";
import { cn } from "@/utils";

// Temporary widgets for testing layout
const TEMP_WIDGETS = [
  { id: "1", type: "test", size: "small" },
  { id: "2", type: "test", size: "medium" },
  { id: "3", type: "test", size: "small" },
  { id: "4", type: "test", size: "large" },
  { id: "5", type: "test", size: "medium" },
] as const;

export default function RightPanel() {
  const { isExpanded } = useRightPanel();

  return (
    <motion.div
      layout
      initial={{ width: 0 }}
      animate={{ width: isExpanded ? 380 : 0 }}
      className={cn("h-full overflow-y-auto border-l bg-neutral-50")}
    >
      <div className="grid grid-cols-[repeat(auto-fill,160px)] justify-center gap-5 p-5">
        {TEMP_WIDGETS.map((widget) => (
          <div
            key={widget.id}
            className={
              widget.size === "medium" || widget.size === "large"
                ? "col-span-2"
                : ""
            }
          >
            <WidgetContainer size={widget.size}>
              Widget {widget.id}
            </WidgetContainer>
          </div>
        ))}
      </div>
    </motion.div>
  );
}
