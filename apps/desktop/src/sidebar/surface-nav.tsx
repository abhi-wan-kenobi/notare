import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import {
  LayoutTemplateIcon,
  CalendarDaysIcon,
  type LucideIcon,
  NotebookTextIcon,
  SettingsIcon,
  UsersIcon,
} from "lucide-react";
import { useCallback } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

type SurfaceId = "notes" | "calendar" | "contacts" | "templates" | "settings";

const SPECIAL_SURFACE_TYPES: Tab["type"][] = [
  "calendar",
  "contacts",
  "templates",
  "settings",
];

export const surfaceFromTabType = (
  type: Tab["type"] | undefined,
): SurfaceId => {
  switch (type) {
    case "calendar":
      return "calendar";
    case "contacts":
      return "contacts";
    case "templates":
      return "templates";
    case "settings":
      return "settings";
    default:
      return "notes";
  }
};

const isMacosPlatform = (): boolean => {
  try {
    return platform() === "macos";
  } catch {
    return false;
  }
};

export function SidebarSurfaceNav() {
  const { t } = useLingui();
  const activeSurface = useTabs((state) =>
    surfaceFromTabType(state.currentTab?.type),
  );

  const goToNotes = useCallback(() => {
    const { tabs, currentTab, select, openCurrent } = useTabs.getState();

    if (!currentTab || !SPECIAL_SURFACE_TYPES.includes(currentTab.type)) {
      return;
    }

    const returnToSlotId = currentTab.returnToSlotId;
    const returnTab = returnToSlotId
      ? tabs.find(
          (tab) =>
            tab.slotId === returnToSlotId &&
            tab.slotId !== currentTab.slotId &&
            (!currentTab.returnToTabId ||
              uniqueIdfromTab(tab) === currentTab.returnToTabId),
        )
      : null;
    if (returnTab) {
      select(returnTab);
      return;
    }

    const existingHomeTab = tabs.find((tab) => tab.type === "empty");
    if (existingHomeTab) {
      select(existingHomeTab);
      return;
    }

    openCurrent({ type: "empty" });
  }, []);

  const openSurface = useCallback((surface: Exclude<SurfaceId, "notes">) => {
    const { openNew } = useTabs.getState();

    switch (surface) {
      case "calendar":
        openNew({ type: "calendar" });
        return;
      case "contacts":
        openNew({ type: "contacts", state: { selected: null } });
        return;
      case "templates":
        openNew({ type: "templates" });
        return;
      case "settings":
        openNew({ type: "settings" });
        return;
    }
  }, []);

  const settingsShortcutHint = isMacosPlatform() ? "⌘ ," : "Ctrl+,";

  const items: {
    id: SurfaceId;
    label: string;
    icon: LucideIcon;
    shortcutHint?: string;
    onClick: () => void;
  }[] = [
    { id: "notes", label: t`Notes`, icon: NotebookTextIcon, onClick: goToNotes },
    {
      id: "calendar",
      label: t`Calendar`,
      icon: CalendarDaysIcon,
      onClick: () => openSurface("calendar"),
    },
    {
      id: "contacts",
      label: t`Contacts`,
      icon: UsersIcon,
      onClick: () => openSurface("contacts"),
    },
    {
      id: "templates",
      label: t`Templates`,
      icon: LayoutTemplateIcon,
      onClick: () => openSurface("templates"),
    },
    {
      id: "settings",
      label: t`Settings`,
      icon: SettingsIcon,
      shortcutHint: settingsShortcutHint,
      onClick: () => openSurface("settings"),
    },
  ];

  return (
    <nav
      aria-label={t`Switch view`}
      data-testid="sidebar-surface-nav"
      data-tauri-drag-region="false"
      className="border-border/80 flex shrink-0 items-center justify-between border-t px-3 py-1.5"
    >
      {items.map((item) => {
        const isActive = activeSurface === item.id;

        return (
          <Tooltip key={item.id} delayDuration={300}>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label={item.label}
                aria-current={isActive ? "page" : undefined}
                data-testid={`sidebar-surface-nav-${item.id}`}
                data-tauri-drag-region="false"
                className={cn([
                  "flex size-7 items-center justify-center rounded-full transition-colors",
                  "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
                  isActive
                    ? "bg-sidebar-accent text-foreground"
                    : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
                ])}
                onClick={item.onClick}
              >
                <item.icon size={15} />
              </button>
            </TooltipTrigger>
            <TooltipContent side="top">
              {item.shortcutHint
                ? `${item.label} (${item.shortcutHint})`
                : item.label}
            </TooltipContent>
          </Tooltip>
        );
      })}
    </nav>
  );
}
