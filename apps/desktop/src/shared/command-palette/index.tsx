import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { Command as CommandPrimitive } from "cmdk";
import { format } from "date-fns";
import {
  CalendarDaysIcon,
  FileTextIcon,
  HomeIcon,
  ListChecksIcon,
  MicIcon,
  MoonIcon,
  NotebookTextIcon,
  SearchIcon,
  SettingsIcon,
  SparklesIcon,
  SunIcon,
  XIcon,
} from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { toTz, useNow, useTimezone } from "~/calendar/hooks";
import { useTimelineEventsTable } from "~/calendar/queries";
import { useRecipes } from "~/chat/recipes";
import { useShell } from "~/contexts/shell";
import { useLiveQuery } from "~/db";
import { useSessionSummaries } from "~/session/queries";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { useMainContentCenterOffset } from "~/shared/main/content-offset";
import {
  applyDocumentTheme,
  writeStoredThemePreference,
} from "~/shared/theme/apply";
import type { ThemePreference } from "~/shared/theme/resolve";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { openSurfaceTab } from "~/sidebar/surface-navigation";
import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

const MAX_RECENT_DISPLAY = 5;

interface OpenNoteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mainContentCenterOffset?: number;
}

type OpenNoteDialogContextValue = { open: () => void };
type SessionLink = {
  session_id: string;
  event_id: string;
  external_event_id: string;
};
type Session = {
  id: string;
  title: string;
  createdAt: string;
};
const EMPTY_SESSION_LINKS: SessionLink[] = [];
const OpenNoteDialogContext = createContext<OpenNoteDialogContextValue | null>(
  null,
);

export function OpenNoteDialogProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const mainContentCenterOffset = useMainContentCenterOffset();
  const openDialog = useCallback(() => setOpen(true), []);

  useHotkeys("mod+k", openDialog, {
    preventDefault: true,
    enableOnFormTags: true,
    enableOnContentEditable: true,
  });

  const value = useMemo(() => ({ open: openDialog }), [openDialog]);

  return (
    <OpenNoteDialogContext.Provider value={value}>
      {children}
      <OpenNoteDialog
        open={open}
        onOpenChange={setOpen}
        mainContentCenterOffset={mainContentCenterOffset}
      />
    </OpenNoteDialogContext.Provider>
  );
}

export function useOpenNoteDialog() {
  const context = useContext(OpenNoteDialogContext);
  if (!context) {
    throw new Error(
      "useOpenNoteDialog must be used within OpenNoteDialogProvider",
    );
  }
  return context;
}

export function OpenNoteDialog({
  open,
  onOpenChange,
  mainContentCenterOffset = 0,
}: OpenNoteDialogProps) {
  const { t } = useLingui();
  const [query, setQuery] = useState("");
  const { inspector } = useShell();
  const openCurrent = useTabs((state) => state.openCurrent);
  const openNew = useTabs((state) => state.openNew);
  const recentlyOpenedSessionIds = useTabs(
    (state) => state.recentlyOpenedSessionIds,
  );
  const createNote = useNewNote({ behavior: "new" });
  const startRecording = useNewNoteAndListen({ behavior: "new" });
  const sessions = useSessionSummaries();
  const { data: sessionLinks = EMPTY_SESSION_LINKS } = useLiveQuery<
    SessionLink,
    SessionLink[]
  >({
    sql: `
      SELECT id AS session_id, event_id, external_event_id
      FROM sessions
      WHERE deleted_at IS NULL
        AND ((event_id IS NOT NULL AND event_id != '')
          OR (external_event_id IS NOT NULL AND external_event_id != ''))
    `,
  });
  const events = useTimelineEventsTable();
  const timezone = useTimezone();
  const now = useNow();
  const { recipes } = useRecipes();
  const theme = useConfigValue("theme") as ThemePreference;
  const setTheme = useSetSettingValue("theme");
  const settingsHubs: { label: string; tab: SettingsTab }[] = [
    { label: t`General & System`, tab: "general" },
    { label: t`AI & Speech`, tab: "ai" },
    { label: t`Integrations & Sync`, tab: "integrations" },
    { label: t`Account & Storage`, tab: "account" },
  ];

  const sessionsMap = useMemo(
    () =>
      new Map<string, Session>(
        sessions.map((session) => [
          session.id,
          {
            id: session.id,
            title: session.title || t`Untitled`,
            createdAt: session.created_at,
          },
        ]),
      ),
    [sessions, t],
  );
  const sessionIdByEventId = useMemo(() => {
    const byEventId = new Map<string, string>();
    const byTrackingId = new Map<string, string>();
    for (const link of sessionLinks) {
      if (link.event_id) byEventId.set(link.event_id, link.session_id);
      if (link.external_event_id) {
        byTrackingId.set(link.external_event_id, link.session_id);
      }
    }
    return { byEventId, byTrackingId };
  }, [sessionLinks]);
  const sortedSessions = useMemo(
    () =>
      Array.from(sessionsMap.values()).sort((a, b) =>
        b.createdAt.localeCompare(a.createdAt),
      ),
    [sessionsMap],
  );
  const recentSessions = useMemo(
    () =>
      recentlyOpenedSessionIds
        .slice(0, MAX_RECENT_DISPLAY)
        .map((id) => sessionsMap.get(id))
        .filter((session): session is Session => session !== undefined),
    [recentlyOpenedSessionIds, sessionsMap],
  );
  const recentSessionIds = useMemo(
    () => new Set(recentSessions.map((session) => session.id)),
    [recentSessions],
  );
  const otherSessions = useMemo(
    () => sortedSessions.filter((session) => !recentSessionIds.has(session.id)),
    [recentSessionIds, sortedSessions],
  );
  const todayEvents = useMemo(() => {
    const today = format(toTz(now, timezone), "yyyy-MM-dd");
    return Object.entries(events ?? {})
      .filter(([, event]) => {
        if (!event.title || !event.started_at) return false;
        return format(toTz(event.started_at, timezone), "yyyy-MM-dd") === today;
      })
      .sort((a, b) =>
        String(a[1].started_at).localeCompare(String(b[1].started_at)),
      );
  }, [events, now, timezone]);

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) setQuery("");
      onOpenChange(nextOpen);
    },
    [onOpenChange],
  );
  const run = useCallback(
    (action: () => void) => {
      handleOpenChange(false);
      action();
    },
    [handleOpenChange],
  );
  const selectSession = useCallback(
    (sessionId: string) =>
      run(() => openCurrent({ type: "sessions", id: sessionId })),
    [openCurrent, run],
  );
  const openSettings = useCallback(
    (tab: SettingsTab) =>
      run(() => openNew({ type: "settings", state: { tab } })),
    [openNew, run],
  );
  const toggleTheme = useCallback(() => {
    const next: ThemePreference = theme === "dark" ? "light" : "dark";
    writeStoredThemePreference(next);
    applyDocumentTheme(next);
    setTheme(next);
  }, [setTheme, theme]);
  const toggleThemeLabel =
    theme === "dark" ? t`Switch to light theme` : t`Switch to dark theme`;
  const shortcut = useMemo(() => {
    try {
      return platform() === "macos" ? "⌘" : "Ctrl+";
    } catch {
      return "Ctrl+";
    }
  }, []);
  const todayLabel = format(toTz(now, timezone), "PP");

  if (!open) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-50 bg-black/20 backdrop-blur-xs"
      onClick={() => handleOpenChange(false)}
    >
      <div
        data-tauri-drag-region
        className="absolute top-0 right-0 left-0 h-[15%]"
        onClick={(event) => event.stopPropagation()}
      />
      <div
        className="absolute top-[15%] left-1/2 w-full max-w-[540px] -translate-x-1/2 px-4"
        style={{ marginLeft: mainContentCenterOffset }}
      >
        <div
          className={cn([
            "border-border/80 bg-background rounded-xl border",
            "overflow-hidden shadow-md",
          ])}
          onClick={(event) => event.stopPropagation()}
        >
          <CommandPrimitive
            shouldFilter
            className="flex flex-col"
            onKeyDown={(event) => {
              if (event.key === "Escape") handleOpenChange(false);
            }}
          >
            <div className="border-border/60 flex items-center gap-3 border-b px-4 py-3">
              <SearchIcon className="text-muted-foreground h-4 w-4 shrink-0" />
              <CommandPrimitive.Input
                autoFocus
                value={query}
                onValueChange={setQuery}
                placeholder={t`Search notes, pages, and actions…`}
                className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm outline-hidden"
              />
              <kbd className="text-muted-foreground border-border rounded border px-1.5 py-0.5 text-[10px]">
                Esc
              </kbd>
              <button
                aria-label={t`Close`}
                onClick={() => handleOpenChange(false)}
                className="bg-accent/80 text-muted-foreground hover:bg-accent flex h-5 w-5 items-center justify-center rounded-full text-xs transition-colors"
              >
                <XIcon className="h-3 w-3" />
              </button>
            </div>
            <CommandPrimitive.List className="max-h-[min(70vh,32rem)] overflow-y-auto p-2">
              <CommandPrimitive.Empty className="text-muted-foreground py-6 text-center text-sm">
                <Trans>No matching commands or notes.</Trans>
              </CommandPrimitive.Empty>

              <CommandPrimitive.Group
                heading={
                  <GroupHeading>
                    <Trans>Actions</Trans>
                  </GroupHeading>
                }
              >
                <PaletteItem
                  value={t`New note`}
                  shortcut={`${shortcut}N`}
                  icon={<FileTextIcon />}
                  onSelect={() => run(createNote)}
                />
                <PaletteItem
                  value={t`Start recording`}
                  shortcut={`⇧${shortcut}N`}
                  icon={<MicIcon />}
                  onSelect={() => run(startRecording)}
                />
                <PaletteItem
                  value={t`Open Home`}
                  shortcut={`${shortcut}1`}
                  icon={<HomeIcon />}
                  onSelect={() => run(() => openSurfaceTab("home"))}
                />
                <PaletteItem
                  value={t`Open Meetings`}
                  shortcut={`${shortcut}2`}
                  icon={<NotebookTextIcon />}
                  onSelect={() => run(() => openSurfaceTab("meetings"))}
                />
                <PaletteItem
                  value={t`Open Calendar`}
                  shortcut={`${shortcut}3`}
                  icon={<CalendarDaysIcon />}
                  onSelect={() => run(() => openSurfaceTab("calendar"))}
                />
                <PaletteItem
                  value={t`Open Action items`}
                  shortcut={`${shortcut}4`}
                  icon={<ListChecksIcon />}
                  onSelect={() => run(() => openSurfaceTab("action-items"))}
                />
                <PaletteItem
                  value={t`Toggle inspector`}
                  shortcut={`${shortcut}J`}
                  icon={<SparklesIcon />}
                  onSelect={() => run(() => inspector.toggle())}
                />
                <PaletteItem
                  value={t`Toggle theme`}
                  label={toggleThemeLabel}
                  icon={theme === "dark" ? <SunIcon /> : <MoonIcon />}
                  onSelect={() => run(toggleTheme)}
                />
                {settingsHubs.map(({ label, tab }) => (
                  <PaletteItem
                    key={tab}
                    value={`${t`Open settings`} ${label}`}
                    label={`${t`Open settings`} · ${label}`}
                    shortcut={tab === "general" ? `${shortcut},` : undefined}
                    icon={<SettingsIcon />}
                    onSelect={() => openSettings(tab)}
                  />
                ))}
                <PaletteItem
                  value={t`Search everything`}
                  icon={<SearchIcon />}
                  onSelect={() =>
                    run(() =>
                      openNew({
                        type: "search",
                        query: query.trim() || undefined,
                      }),
                    )
                  }
                />
              </CommandPrimitive.Group>

              {(recentSessions.length > 0 ||
                otherSessions.length > 0 ||
                todayEvents.length > 0) && (
                <CommandPrimitive.Group
                  heading={
                    <GroupHeading>
                      <Trans>Meetings</Trans>
                    </GroupHeading>
                  }
                >
                  {recentSessions.map((session) => (
                    <PaletteItem
                      key={`recent-${session.id}`}
                      value={`${session.title} ${formatSessionDate(session.createdAt)}`}
                      label={session.title}
                      date={formatSessionDate(session.createdAt)}
                      icon={<NotebookTextIcon />}
                      onSelect={() => selectSession(session.id)}
                    />
                  ))}
                  {otherSessions.map((session) => (
                    <PaletteItem
                      key={session.id}
                      value={`${session.title} ${formatSessionDate(session.createdAt)}`}
                      label={session.title}
                      date={formatSessionDate(session.createdAt)}
                      icon={<NotebookTextIcon />}
                      onSelect={() => selectSession(session.id)}
                    />
                  ))}
                  {todayEvents.map(([eventId, event]) => {
                    const time = format(toTz(event.started_at!, timezone), "p");
                    return (
                      <PaletteItem
                        key={`event-${eventId}`}
                        value={`${event.title} ${todayLabel} ${time}`}
                        label={event.title!}
                        icon={<CalendarDaysIcon />}
                        shortcut={time}
                        onSelect={() => {
                          const linkedSessionId =
                            sessionIdByEventId.byEventId.get(eventId) ??
                            (event.tracking_id_event
                              ? sessionIdByEventId.byTrackingId.get(
                                  event.tracking_id_event,
                                )
                              : undefined);
                          if (linkedSessionId) {
                            selectSession(linkedSessionId);
                          } else {
                            run(() => openSurfaceTab("calendar"));
                          }
                        }}
                      />
                    );
                  })}
                  <PaletteItem
                    value={t`All meetings`}
                    icon={<NotebookTextIcon />}
                    onSelect={() => run(() => openSurfaceTab("meetings"))}
                  />
                  <PaletteItem
                    value={t`Action items`}
                    icon={<ListChecksIcon />}
                    onSelect={() => run(() => openSurfaceTab("action-items"))}
                  />
                </CommandPrimitive.Group>
              )}

              {recipes.length > 0 && (
                <CommandPrimitive.Group
                  heading={
                    <GroupHeading>
                      <Trans>Recipes</Trans>
                    </GroupHeading>
                  }
                >
                  {recipes.map((recipe) => (
                    <PaletteItem
                      key={recipe.id}
                      value={`${recipe.title} ${recipe.prompt}`}
                      label={recipe.title}
                      icon={<SparklesIcon />}
                      shortcut="↵"
                      onSelect={() =>
                        run(() => inspector.openWith(recipe.prompt))
                      }
                    />
                  ))}
                </CommandPrimitive.Group>
              )}
            </CommandPrimitive.List>
          </CommandPrimitive>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function GroupHeading({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
      {children}
    </div>
  );
}

function PaletteItem({
  value,
  label = value,
  date,
  shortcut,
  icon,
  onSelect,
}: {
  value: string;
  label?: string;
  date?: string;
  shortcut?: string;
  icon: React.ReactNode;
  onSelect: () => void;
}) {
  return (
    <CommandPrimitive.Item
      value={value}
      onSelect={onSelect}
      className={cn([
        "flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-1.75 text-sm",
        "text-foreground data-[selected=true]:bg-accent/60",
        "transition-colors",
      ])}
    >
      <span className="text-muted-foreground [&_svg]:size-4 [&_svg]:shrink-0">
        {icon}
      </span>
      <span className="truncate">{label}</span>
      {date && (
        <span className="text-muted-foreground shrink-0 text-xs tabular-nums">
          · {date}
        </span>
      )}
      <span className="flex-1" />
      {shortcut && (
        <kbd className="text-muted-foreground border-border rounded border px-1.5 py-0.5 font-mono text-[10px]">
          {shortcut}
        </kbd>
      )}
    </CommandPrimitive.Item>
  );
}

function formatSessionDate(value: string): string {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? "" : format(parsed, "MMM d");
}
