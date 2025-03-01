import "react-grid-layout/css/styles.css";
import "react-resizable/css/styles.css";

import { useCallback, useState } from "react";
import GridLayout, { Layout } from "react-grid-layout";

import { WidgetGroup } from "@hypr/extension-utils";

import DinoExtension from "@hypr/extension-dino-game";
import ClockExtension from "@hypr/extension-clock";
import SummaryExtension from "@hypr/extension-summary";

const getTwoByOne = (group: WidgetGroup) => {
  const item = group.items.find((item) => item.type === "twoByOne");
  if (!item) {
    return null;
  }
  return item.component;
};

const getTwoByTwo = (group: WidgetGroup) => {
  const item = group.items.find((item) => item.type === "twoByTwo");
  if (!item) {
    return null;
  }
  return item.component;
};

const CC = getTwoByOne(DinoExtension["chromeDino"])!;
const CC2 = getTwoByTwo(SummaryExtension["live"])!;
const CC3 = getTwoByTwo(ClockExtension["world"])!;

export default function WidgetRenderer() {
  const [layout, setLayout] = useState<Layout[]>([
    {
      i: "1",
      x: 0,
      y: 0,
      w: 2,
      h: 1,
    },
    {
      i: "2",
      x: 0,
      y: 0,
      w: 2,
      h: 2,
    },
    {
      i: "3",
      x: 0,
      y: 0,
      w: 2,
      h: 2,
    },
  ]);

  const handleLayoutChange = useCallback((newLayout: Layout[]) => {
    setLayout(newLayout);
  }, []);

  return (
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
      <div key="1">
        <CC />
      </div>
      <div key="2">
        <CC2 />
      </div>
      <div key="3">
        <CC3 />
      </div>
    </GridLayout>
  );
}
