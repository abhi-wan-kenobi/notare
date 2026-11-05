import { TabContentHome } from "./home";

import { MainTabContent } from "~/shared/main/tab-content";
import { type Tab } from "~/store/zustand/tabs";

export function ClassicMainTabContent({ tab }: { tab: Tab }) {
  if (tab.type === "empty") {
    return <TabContentHome tab={tab} />;
  }

  return <MainTabContent tab={tab} />;
}
