import { type Tab, useTabs } from "~/store/zustand/tabs";

export type SurfaceId =
  | "home"
  | "meetings"
  | "calendar"
  | "action-items"
  | "templates"
  | "snippets"
  | "contacts"
  | "trash"
  | "settings";

export const surfaceFromTabType = (
  type: Tab["type"] | undefined,
): SurfaceId | undefined => {
  switch (type) {
    case "empty":
      return "home";
    case "sessions":
    case "meetings":
    case "series":
      return "meetings";
    case "calendar":
      return "calendar";
    case "action-items":
      return "action-items";
    case "templates":
      return "templates";
    case "snippets":
      return "snippets";
    case "contacts":
      return "contacts";
    case "trash":
      return "trash";
    case "settings":
      return "settings";
    default:
      return undefined;
  }
};

export const openSurfaceTab = (surface: SurfaceId) => {
  const { openNew, openCurrent, select, tabs } = useTabs.getState();

  switch (surface) {
    case "home": {
      const existingHomeTab = tabs.find((tab) => tab.type === "empty");
      if (existingHomeTab) {
        select(existingHomeTab);
        return;
      }
      openCurrent({ type: "empty" });
      return;
    }
    case "meetings":
      openNew({ type: "meetings" });
      return;
    case "calendar":
      openNew({ type: "calendar" });
      return;
    case "action-items":
      openNew({ type: "action-items" });
      return;
    case "templates":
      openNew({ type: "templates" });
      return;
    case "snippets":
      openNew({ type: "snippets" });
      return;
    case "contacts":
      openNew({ type: "contacts", state: { selected: null } });
      return;
    case "trash":
      openNew({ type: "trash" });
      return;
    case "settings":
      openNew({ type: "settings" });
      return;
  }
};
