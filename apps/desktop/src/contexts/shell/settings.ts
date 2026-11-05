import { useCallback } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

export function useSettings() {
  const openNew = useTabs((state) => state.openNew);
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );

  const openSettings = useCallback(
    (target?: { tab?: SettingsTab; section?: string }) => {
      const state = {
        tab: target?.tab ?? "general",
        section: target?.section ?? null,
      };
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, state);
      } else {
        openNew({ type: "settings", state });
      }
    },
    [currentTab, openNew, updateSettingsTabState],
  );

  useHotkeys(
    "mod+,",
    () => openSettings(),
    {
      preventDefault: true,
      splitKey: "|",
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openSettings],
  );

  return { openSettings };
}
