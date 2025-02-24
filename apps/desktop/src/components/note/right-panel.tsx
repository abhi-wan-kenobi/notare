import { motion } from "motion/react";
import { useRightPanel } from "@/contexts/right-panel";
import { WidgetContainer } from "./widgets/widget-container";
import { useState, useCallback } from "react";
import type { Widget } from "./widgets/types";
import GridLayout, { Layout } from "react-grid-layout";
import "react-grid-layout/css/styles.css";
import "react-resizable/css/styles.css";

// Temporary widgets for testing layout
const INITIAL_WIDGETS: Widget[] = [
  { id: "1", type: "test", size: "small" },
  { id: "2", type: "test", size: "medium" },
  { id: "3", type: "test", size: "small" },
  { id: "4", type: "test", size: "large" },
  { id: "5", type: "test", size: "medium" },
];

// Convert widget size to grid dimensions
const sizeToDimensions = {
  small: { w: 1, h: 1 },
  medium: { w: 2, h: 1 },
  large: { w: 2, h: 2 },
};

export default function RightPanel() {
  const { isExpanded } = useRightPanel();
  const [layout, setLayout] = useState<Layout[]>(
    INITIAL_WIDGETS.map((widget, i) => ({
      i: widget.id,
      x: 0,
      y: i * 2,
      ...sizeToDimensions[widget.size],
      static: false,
    })),
  );

  const handleLayoutChange = useCallback((newLayout: Layout[]) => {
    setLayout(newLayout);
  }, []);

  return (
    <motion.div
      layout
      initial={{ width: 0 }}
      animate={{ width: isExpanded ? 380 : 0 }}
      className="h-full overflow-y-auto border-l bg-neutral-50"
    >
      <GridLayout
        className="layout"
        layout={layout}
        cols={2}
        rowHeight={160}
        width={380}
        margin={[20, 20]}
        onLayoutChange={handleLayoutChange}
        isDraggable
        isResizable={false}
        compactType="vertical"
      >
        {INITIAL_WIDGETS.map((widget) => (
          <div key={widget.id}>
            <WidgetContainer size={widget.size}>
              <div className="flex h-full items-center justify-center">
                Widget {widget.id}
                <br />
                Drag to reorder
              </div>
            </WidgetContainer>
          </div>
        ))}
      </GridLayout>
    </motion.div>
  );
}
