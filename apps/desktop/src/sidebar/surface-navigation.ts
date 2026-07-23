import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

export type SurfaceId =
  | "notes"
  | "search"
  | "calendar"
  | "contacts"
  | "templates"
  | "settings";

export const SPECIAL_SURFACE_TYPES: Tab["type"][] = [
  "calendar",
  "contacts",
  "templates",
  "settings",
];

export const surfaceFromTabType = (
  type: Tab["type"] | undefined,
): SurfaceId => {
  switch (type) {
    case "search":
      return "search";
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

/**
 * Rail "Notes" click: leave the current special surface and land back on a
 * notes-family tab. The return origin must never be another special surface
 * (that would make the Notes button navigate to e.g. Templates), so origins
 * that fail that check fall through to the home tab.
 */
export const goToNotesSurface = () => {
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
          !SPECIAL_SURFACE_TYPES.includes(tab.type) &&
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
};

export const openSurfaceTab = (surface: Exclude<SurfaceId, "notes">) => {
  const { openNew } = useTabs.getState();

  switch (surface) {
    case "search":
      openNew({ type: "search" });
      return;
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
};
