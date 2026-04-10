import { CalendarIcon } from "lucide-react";

import { TabItemBase, type TabItem } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

type DailySummaryTab = Extract<Tab, { type: "daily_summary" }>;

function formatDateHeader(dateStr: string): string {
  const date = new Date(dateStr + "T00:00:00");
  const month = date.toLocaleDateString("en-US", { month: "short" });
  const day = date.getDate();
  return `${month} ${day}`;
}

export const TabItemDailySummary: TabItem<DailySummaryTab> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<CalendarIcon className="h-4 w-4" />}
      title={formatDateHeader(tab.id)}
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};
