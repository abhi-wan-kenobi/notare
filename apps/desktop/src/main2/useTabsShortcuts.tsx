import { getCurrentWindow } from "@tauri-apps/api/window";
import { useHotkeys } from "react-hotkeys-hook";

import { useMainTabsShortcuts } from "~/shared/useTabsShortcuts";
import { useTabs } from "~/store/zustand/tabs";

export function useMain2TabsShortcuts() {
  const clearSelection = useTabs((state) => state.clearSelection);
  const currentTab = useTabs((state) => state.currentTab);

  useHotkeys(
    "mod+w",
    () => {
      if (!currentTab) {
        getCurrentWindow().close();
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab],
  );

  return useMainTabsShortcuts({ onModT: clearSelection });
}
