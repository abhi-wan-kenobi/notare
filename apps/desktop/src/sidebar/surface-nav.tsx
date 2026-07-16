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

import {
  goToNotesSurface,
  openSurfaceTab,
  surfaceFromTabType,
  type SurfaceId,
} from "~/sidebar/surface-navigation";
import { useTabs } from "~/store/zustand/tabs";

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
    goToNotesSurface();
  }, []);

  const openSurface = useCallback((surface: Exclude<SurfaceId, "notes">) => {
    openSurfaceTab(surface);
  }, []);

  const settingsShortcutHint = isMacosPlatform() ? "⌘ ," : "Ctrl+,";

  const items: {
    id: SurfaceId;
    label: string;
    icon: LucideIcon;
    shortcutHint?: string;
    onClick: () => void;
  }[] = [
    {
      id: "notes",
      label: t`Notes`,
      icon: NotebookTextIcon,
      onClick: goToNotes,
    },
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
