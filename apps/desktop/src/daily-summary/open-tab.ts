import { id } from "~/shared/utils";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function openDailySummaryTab(date: string) {
  const state = useTabs.getState();
  const { tabs } = state;

  const existing = tabs.find(
    (tab) => tab.type === "daily_summary" && tab.id === date,
  );
  if (existing) {
    state.select(existing);
    return;
  }

  const newTab: Tab = {
    type: "daily_summary",
    id: date,
    active: true,
    slotId: id(),
    pinned: false,
  };

  const deactivated = tabs.map((tab) => ({ ...tab, active: false }));
  useTabs.setState({
    tabs: [...deactivated, newTab],
    currentTab: newTab,
  });
}
