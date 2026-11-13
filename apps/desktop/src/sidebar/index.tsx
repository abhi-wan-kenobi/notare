import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import {
  CalendarDaysIcon,
  CheckSquareIcon,
  ChevronRightIcon,
  HomeIcon,
  LayoutTemplateIcon,
  NotebookTextIcon,
  ScrollTextIcon,
  SearchIcon,
  SettingsIcon,
  Trash2Icon,
  UsersIcon,
} from "lucide-react";
import { type ReactNode, useCallback, useMemo, useState } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { CalendarNav } from "./calendar";
import { ContactsNav } from "./contacts";
import { SettingsNav } from "./settings";
import { openSurfaceTab, surfaceFromTabType } from "./surface-navigation";
import { TemplatesNav } from "./templates";

import { useLiveQuery } from "~/db";
import { useSessionSummaries } from "~/session/queries";
import { useOpenNoteDialog } from "~/shared/command-palette";
import { useTabs } from "~/store/zustand/tabs";

const MAX_RECENT_DISPLAY = 5;
const LIBRARY_COLLAPSED_STORAGE_KEY = "notare.sidebar.library_collapsed";

const isMacosPlatform = (): boolean => {
  try {
    return platform() === "macos";
  } catch {
    return false;
  }
};

function readStoredLibraryCollapsed(): boolean {
  try {
    const stored = localStorage.getItem(LIBRARY_COLLAPSED_STORAGE_KEY);
    return stored === null ? true : stored === "true";
  } catch {
    return true;
  }
}

export function LeftSidebar({
  timelineHeader,
}: {
  timelineHeader?: ReactNode;
} = {}) {
  const currentTab = useTabs((state) => state.currentTab);

  const isSettingsMode = currentTab?.type === "settings";
  const isCalendarMode = currentTab?.type === "calendar";
  const isContactsMode = currentTab?.type === "contacts";
  const isTemplatesMode = currentTab?.type === "templates";
  const isSpecialMode =
    isSettingsMode || isCalendarMode || isContactsMode || isTemplatesMode;
  const isTimelineSidebarLayout = !isSpecialMode;

  return (
    <div
      className={cn([
        "flex h-full w-full shrink-0 flex-col gap-1 overflow-hidden",
        isTimelineSidebarLayout ? "pt-0" : "pt-11",
        !isTimelineSidebarLayout && "pr-1",
      ])}
    >
      <div className="flex flex-1 flex-col gap-1 overflow-hidden">
        {isTimelineSidebarLayout ? timelineHeader : null}
        <div className="relative min-h-0 flex-1 overflow-hidden">
          {isSettingsMode ? (
            <SettingsNav />
          ) : isCalendarMode ? (
            <CalendarNav />
          ) : isContactsMode ? (
            <ContactsNav />
          ) : isTemplatesMode ? (
            <TemplatesNav />
          ) : (
            <DefaultSidebarContent />
          )}
        </div>
      </div>
    </div>
  );
}

function DefaultSidebarContent() {
  const { t } = useLingui();
  const currentTab = useTabs((state) => state.currentTab);
  const activeSurface = surfaceFromTabType(currentTab?.type);
  const openNoteDialog = useOpenNoteDialog();
  const [libraryCollapsed, setLibraryCollapsed] = useState(
    readStoredLibraryCollapsed,
  );

  const { data: openActionItemsCount = 0 } = useLiveQuery<
    { count: number },
    number
  >({
    sql: `
      SELECT COUNT(*) AS count
      FROM action_items
      WHERE deleted_at IS NULL AND status != 'done'
    `,
    mapRows: (rows) => Number(rows[0]?.count ?? 0),
  });

  const toggleLibrary = useCallback(() => {
    setLibraryCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(LIBRARY_COLLAPSED_STORAGE_KEY, String(next));
      } catch {
        // localStorage unavailable (tests) — in-memory state still updates.
      }
      return next;
    });
  }, []);

  const settingsShortcutHint = isMacosPlatform() ? "⌘ ," : "Ctrl+,";

  const primaryRows: {
    id: "home" | "meetings" | "calendar" | "action-items";
    label: string;
    icon: typeof HomeIcon;
    badge?: number;
  }[] = [
    { id: "home", label: t`Home`, icon: HomeIcon },
    { id: "meetings", label: t`Meetings`, icon: NotebookTextIcon },
    { id: "calendar", label: t`Calendar`, icon: CalendarDaysIcon },
    {
      id: "action-items",
      label: t`Action items`,
      icon: CheckSquareIcon,
      badge: openActionItemsCount,
    },
  ];

  const libraryRows: {
    id: "templates" | "snippets" | "contacts" | "trash";
    label: string;
    icon: typeof LayoutTemplateIcon;
  }[] = [
    { id: "templates", label: t`Templates`, icon: LayoutTemplateIcon },
    { id: "snippets", label: t`Snippets`, icon: ScrollTextIcon },
    { id: "contacts", label: t`Contacts`, icon: UsersIcon },
    { id: "trash", label: t`Trash`, icon: Trash2Icon },
  ];

  return (
    <div
      data-testid="sidebar-default-content"
      className="flex h-full min-h-0 flex-col gap-2.5 overflow-hidden px-2.5 pt-1"
    >
      <div className="font-serif text-[17px] font-semibold tracking-tight">
        Notare
      </div>

      <button
        type="button"
        onClick={() => openNoteDialog.open()}
        className="border-border bg-card text-muted-foreground flex h-9 shrink-0 items-center gap-2 rounded-lg border px-2.5 text-xs"
      >
        <SearchIcon size={14} />
        <span className="flex-1 truncate text-left">
          <Trans>Search notes and actions</Trans>
        </span>
        <Kbd>{isMacosPlatform() ? "⌘K" : "Ctrl+K"}</Kbd>
      </button>

      <nav aria-label={t`Primary`} className="flex flex-col gap-0.5">
        {primaryRows.map((row) => (
          <SidebarNavRow
            key={row.id}
            label={row.label}
            icon={row.icon}
            active={activeSurface === row.id}
            badge={row.badge}
            onClick={() => openSurfaceTab(row.id)}
          />
        ))}
      </nav>

      <RecentSection activeSurface={activeSurface} />

      <div className="min-h-0 shrink-0">
        <button
          type="button"
          data-library-toggle
          onClick={toggleLibrary}
          aria-expanded={!libraryCollapsed}
          className="text-muted-foreground hover:bg-accent/50 flex h-8 w-full items-center gap-2 rounded-md px-2 text-xs"
        >
          <ChevronRightIcon
            size={14}
            className={cn([
              "transition-transform",
              !libraryCollapsed && "rotate-90",
            ])}
          />
          <Trans>Library</Trans>
        </button>
        {libraryCollapsed ? null : (
          <div className="flex flex-col gap-0.5 pl-3">
            {libraryRows.map((row) => (
              <SidebarNavRow
                key={row.id}
                label={row.label}
                icon={row.icon}
                active={activeSurface === row.id}
                onClick={() => openSurfaceTab(row.id)}
                compact
              />
            ))}
          </div>
        )}
      </div>

      <div className="border-border shrink-0 border-t pt-2 pb-2">
        <SidebarNavRow
          label={t`Settings`}
          icon={SettingsIcon}
          active={activeSurface === "settings"}
          onClick={() => openSurfaceTab("settings")}
          shortcutHint={settingsShortcutHint}
        />
      </div>
    </div>
  );
}

function RecentSection({
  activeSurface,
}: {
  activeSurface: ReturnType<typeof surfaceFromTabType>;
}) {
  const { t } = useLingui();
  const recentlyOpenedSessionIds = useTabs(
    (state) => state.recentlyOpenedSessionIds,
  );
  const sessions = useSessionSummaries();
  const openCurrent = useTabs((state) => state.openCurrent);
  const currentSessionId = useTabs((state) =>
    state.currentTab?.type === "sessions" ? state.currentTab.id : undefined,
  );

  const sessionsMap = useMemo(
    () => new Map(sessions.map((session) => [session.id, session])),
    [sessions],
  );

  const recentSessions = useMemo(
    () =>
      recentlyOpenedSessionIds
        .slice(0, MAX_RECENT_DISPLAY)
        .map((id) => sessionsMap.get(id))
        .filter((session): session is NonNullable<typeof session> =>
          Boolean(session),
        ),
    [recentlyOpenedSessionIds, sessionsMap],
  );

  if (recentSessions.length === 0) {
    return null;
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="text-tertiary px-2 pt-1 pb-1 text-[10px] font-semibold tracking-wide uppercase">
        <Trans>Recent</Trans>
      </div>
      <div className="flex flex-col gap-0.5">
        {recentSessions.map((session) => (
          <button
            key={session.id}
            type="button"
            onClick={() => openCurrent({ type: "sessions", id: session.id })}
            className={cn([
              "flex h-[34px] w-full items-center rounded-md px-2 text-left text-xs",
              activeSurface === "meetings" && session.id === currentSessionId
                ? "bg-sidebar-accent text-foreground"
                : "text-foreground/90 hover:bg-accent/50",
            ])}
          >
            <span className="flex-1 truncate">
              {session.title || t`Untitled`}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function SidebarNavRow({
  label,
  icon: Icon,
  active,
  badge,
  shortcutHint,
  compact = false,
  onClick,
}: {
  label: string;
  icon: typeof HomeIcon;
  active: boolean;
  badge?: number;
  shortcutHint?: string;
  compact?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      className={cn([
        "flex w-full items-center gap-2.5 rounded-md px-2 text-left text-[13px]",
        compact ? "h-8" : "h-9",
        active
          ? "bg-sidebar-accent text-foreground font-medium"
          : "text-foreground/90 hover:bg-accent/50",
      ])}
    >
      <Icon size={15} className="text-muted-foreground shrink-0" />
      <span className="flex-1 truncate">{label}</span>
      {badge ? (
        <span className="text-muted-foreground text-[11px]">{badge}</span>
      ) : null}
      {shortcutHint ? <Kbd className="text-[10px]">{shortcutHint}</Kbd> : null}
    </button>
  );
}
