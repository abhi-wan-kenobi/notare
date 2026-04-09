import { SquareCheckBigIcon } from "lucide-react";

import { TabItemBase, type TabItem } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

type TaskTab = Extract<Tab, { type: "task" }>;

export const TabItemTask: TabItem<TaskTab> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  const first = tab.resources[0];
  const title = first
    ? `${first.owner}/${first.repo} #${first.number}`
    : "Task";

  return (
    <TabItemBase
      icon={<SquareCheckBigIcon className="h-4 w-4" />}
      title={title}
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
