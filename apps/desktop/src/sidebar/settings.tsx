import { Trans } from "@lingui/react/macro";
import {
  AudioLinesIcon,
  CogIcon,
  HardDriveIcon,
  PlugZapIcon,
} from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { resolveSettingsHub } from "~/settings/hubs";
import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

const HUBS: {
  id: SettingsTab;
  label: ReactNode;
  icon: typeof CogIcon;
}[] = [
  { id: "general", label: <Trans>General &amp; System</Trans>, icon: CogIcon },
  {
    id: "ai",
    label: <Trans>AI &amp; Speech</Trans>,
    icon: AudioLinesIcon,
  },
  {
    id: "integrations",
    label: <Trans>Integrations &amp; Sync</Trans>,
    icon: PlugZapIcon,
  },
  {
    id: "account",
    label: <Trans>Account &amp; Storage</Trans>,
    icon: HardDriveIcon,
  },
];

export function SettingsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const activeTab =
    currentTab?.type === "settings"
      ? resolveSettingsHub(currentTab.state?.tab)
      : "general";

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <CustomSidebarHeader title={<Trans>Settings</Trans>} />
      <nav aria-label="Settings hubs" className="flex flex-col gap-1 px-2 py-3">
        {HUBS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            aria-current={activeTab === id ? "page" : undefined}
            onClick={() => {
              if (currentTab?.type === "settings") {
                updateSettingsTabState(currentTab, { tab: id });
              }
            }}
            className={cn([
              "flex min-h-9 w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
              activeTab === id
                ? "bg-sidebar-accent text-foreground font-medium"
                : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
            ])}
          >
            <Icon size={15} className="shrink-0" />
            <span className="min-w-0 flex-1 truncate">{label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
