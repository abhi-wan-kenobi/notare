import type {
  ChangelogState,
  ContactsSelection,
  ContactsState,
  EditorView,
  SessionsState,
  TabInput as WindowsTabInput,
  TemplatesState,
} from "@hypr/plugin-windows";

export type {
  ChangelogState,
  ContactsSelection,
  ContactsState,
  EditorView,
  SessionsState,
  TemplatesState,
};

/** The subset of the Rust/windows-generated tab inputs the desktop app renders. */
export type WindowsSupportedTabInput = Exclude<
  WindowsTabInput,
  { type: "extension" } | { type: "extensions" } | { type: "folders" }
>;

export type TabInput =
  | Exclude<WindowsSupportedTabInput, { type: "settings" }>
  | { type: "settings"; state?: SettingsInputState | null }
  // App-only tab input (no Rust/windows binding): the hybrid search surface.
  | { type: "search"; query?: string }
  // App-only tab input (no Rust/windows binding): the Snippets (dictation
  // history) surface, promoted out of Settings > Dictation.
  | { type: "snippets" }
  // App-only tab input (no Rust/windows binding): the Trash (trashed
  // sessions) surface.
  | { type: "trash" }
  // App-only tab input (no Rust/windows binding): the Meetings list surface.
  | { type: "meetings" }
  // App-only tab input (no Rust/windows binding): the Action items list
  // surface, aggregating action items across sessions.
  | { type: "action-items" }
  // App-only tab input (no Rust/windows binding): a recurring-event series'
  // occurrence timeline.
  | { type: "series"; seriesId: string };

export const isTabInputSupported = (
  tab: WindowsTabInput,
): tab is WindowsSupportedTabInput => {
  return (
    tab.type !== "extension" &&
    tab.type !== "extensions" &&
    tab.type !== "folders"
  );
};

export type SettingsTab = "general" | "ai" | "integrations" | "account";

export const isSettingsTab = (
  tab: string | null | undefined,
): tab is SettingsTab => {
  switch (tab) {
    case "general":
    case "ai":
    case "integrations":
    case "account":
      return true;
    default:
      return false;
  }
};

export const normalizeSettingsTab = (
  tab: string | null | undefined,
): SettingsTab => {
  switch (tab) {
    case "general":
    case "ai":
    case "integrations":
    case "account":
      return tab;
    case "app":
    case "notifications":
    case "permissions":
    case "personalization":
    case "dictation":
      return "general";
    case "transcription":
    case "intelligence":
    case "recipes":
      return "ai";
    case "webhook":
    case "sync":
    case "mcp":
    case "gmail":
      return "integrations";
    case "todo":
    case "storage":
    case "data":
      return "account";
    default:
      return "general";
  }
};

export type SettingsState = {
  tab: SettingsTab | null;
  section?: string | null;
};

export type SettingsInputState = {
  tab?: string | null;
  section?: string | null;
};

export const normalizeSettingsState = (
  state?: { tab?: string | null; section?: string | null } | null,
): SettingsState => {
  const rawTab = state?.tab;
  const section = state?.section ?? null;
  if (!rawTab) {
    return { tab: "general", section };
  }
  if (isSettingsTab(rawTab)) {
    return { tab: rawTab, section };
  }
  switch (rawTab) {
    case "notifications":
    case "permissions":
    case "personalization":
    case "dictation":
      return { tab: "general", section: section ?? rawTab };
    case "app":
      return { tab: "general", section };
    case "transcription":
    case "recipes":
      return { tab: "ai", section: section ?? rawTab };
    case "intelligence":
      return { tab: "ai", section };
    case "webhook":
    case "sync":
    case "mcp":
    case "gmail":
      return { tab: "integrations", section: section ?? rawTab };
    case "todo":
    case "storage":
    case "data":
      return { tab: "account", section: section ?? rawTab };
    default:
      return { tab: "general", section };
  }
};

export type DailySummaryState = {
  activeTab: "timeline" | "raw" | null;
};

export const isEnhancedView = (
  view: EditorView,
): view is { type: "enhanced"; id: string } => view.type === "enhanced";
export const isRawView = (view: EditorView): view is { type: "raw" } =>
  view.type === "raw";

type BaseTab = {
  active: boolean;
  slotId: string;
  pinned: boolean;
  returnToSlotId?: string;
  returnToTabId?: string;
};

export type Tab =
  | (BaseTab & {
      type: "sessions";
      id: string;
      state: SessionsState;
    })
  | (BaseTab & {
      type: "contacts";
      state: ContactsState;
    })
  | (BaseTab & {
      type: "templates";
      state: TemplatesState;
    })
  | (BaseTab & {
      type: "humans";
      id: string;
    })
  | (BaseTab & { type: "organizations"; id: string })
  | (BaseTab & { type: "empty" })
  | (BaseTab & { type: "calendar" })
  | (BaseTab & { type: "search"; query?: string })
  | (BaseTab & { type: "snippets" })
  | (BaseTab & { type: "trash" })
  | (BaseTab & { type: "meetings" })
  | (BaseTab & { type: "action-items" })
  | (BaseTab & { type: "series"; state: { seriesId: string } })
  | (BaseTab & {
      type: "changelog";
      state: ChangelogState;
    })
  | (BaseTab & { type: "settings"; state: SettingsState })
  | (BaseTab & { type: "onboarding" })
  | (BaseTab & { type: "edit"; requestId: string })
  | (BaseTab & {
      type: "task";
      id: string;
      resources: TaskResource[];
    })
  | (BaseTab & {
      type: "daily_summary";
      id: string;
      state: DailySummaryState;
    });

export type TaskResource =
  | { type: "github_issue"; owner: string; repo: string; number: number }
  | { type: "github_pr"; owner: string; repo: string; number: number };

export const getDefaultState = (tab: TabInput): Tab => {
  const base = { active: false, slotId: "", pinned: false };

  switch (tab.type) {
    case "sessions":
      return {
        ...base,
        type: "sessions",
        id: tab.id,
        state: tab.state ?? { view: null, autoStart: null },
      };
    case "contacts":
      return {
        ...base,
        type: "contacts",
        state: tab.state ?? {
          selected: null,
        },
      };
    case "templates":
      return {
        ...base,
        type: "templates",
        state: tab.state ?? {
          showHomepage: false,
          isWebMode: true,
          selectedMineId: null,
          selectedWebIndex: null,
        },
      };
    case "humans":
      return { ...base, type: "humans", id: tab.id };
    case "organizations":
      return { ...base, type: "organizations", id: tab.id };
    case "empty":
      return { ...base, type: "empty" };
    case "calendar":
      return { ...base, type: "calendar" };
    case "search":
      return { ...base, type: "search", query: tab.query };
    case "snippets":
      return { ...base, type: "snippets" };
    case "trash":
      return { ...base, type: "trash" };
    case "meetings":
      return { ...base, type: "meetings" };
    case "action-items":
      return { ...base, type: "action-items" };
    case "series":
      return {
        ...base,
        type: "series",
        state: { seriesId: tab.seriesId },
      };
    case "changelog":
      return {
        ...base,
        type: "changelog",
        state: tab.state,
      };
    case "settings": {
      const subtab = tab.state?.tab as string | null | undefined;
      if (subtab === "calendar") {
        return { ...base, type: "calendar" };
      }
      return {
        ...base,
        type: "settings",
        state: normalizeSettingsState(
          tab.state as
            | { tab?: string | null; section?: string | null }
            | undefined,
        ),
      };
    }
    case "onboarding":
      return { ...base, type: "onboarding" };
    case "edit":
      return { ...base, type: "edit", requestId: tab.requestId };
    default:
      const _exhaustive: never = tab;
      return _exhaustive;
  }
};

export const uniqueIdfromTab = (tab: Tab): string => {
  switch (tab.type) {
    case "sessions":
      return `sessions-${tab.id}`;
    case "humans":
      return `humans-${tab.id}`;
    case "organizations":
      return `organizations-${tab.id}`;
    case "contacts":
      return `contacts`;
    case "templates":
      return `templates`;
    case "empty":
      return `empty-${tab.slotId}`;
    case "calendar":
      return `calendar`;
    case "search":
      return `search`;
    case "snippets":
      return `snippets`;
    case "trash":
      return `trash`;
    case "meetings":
      return `meetings`;
    case "action-items":
      return `action-items`;
    case "series":
      return `series-${tab.state.seriesId}`;
    case "changelog":
      return "changelog";
    case "settings":
      return `settings`;
    case "onboarding":
      return `onboarding`;
    case "edit":
      return `edit-${tab.requestId}`;
    case "task":
      return `task-${tab.id}`;
    case "daily_summary":
      return `daily_summary-${tab.id}`;
  }
};

export const isSameTab = (a: Tab, b: Tab) => {
  return uniqueIdfromTab(a) === uniqueIdfromTab(b);
};
