import { beforeEach, describe, expect, test } from "vitest";

import { openSurfaceTab, surfaceFromTabType } from "./surface-navigation";

import { uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import {
  createSessionTab,
  createSettingsTab,
  resetTabsStore,
} from "~/store/zustand/tabs/test-utils";

describe("surfaceFromTabType", () => {
  test("maps the empty tab to home", () => {
    expect(surfaceFromTabType("empty")).toBe("home");
  });

  test("maps sessions, meetings and series tabs to meetings", () => {
    expect(surfaceFromTabType("sessions")).toBe("meetings");
    expect(surfaceFromTabType("meetings")).toBe("meetings");
    expect(surfaceFromTabType("series")).toBe("meetings");
  });

  test("maps the remaining primary and library surfaces 1:1", () => {
    expect(surfaceFromTabType("calendar")).toBe("calendar");
    expect(surfaceFromTabType("action-items")).toBe("action-items");
    expect(surfaceFromTabType("templates")).toBe("templates");
    expect(surfaceFromTabType("snippets")).toBe("snippets");
    expect(surfaceFromTabType("contacts")).toBe("contacts");
    expect(surfaceFromTabType("trash")).toBe("trash");
    expect(surfaceFromTabType("settings")).toBe("settings");
  });

  test("returns undefined for tab types with no sidebar row", () => {
    expect(surfaceFromTabType("search")).toBeUndefined();
    expect(surfaceFromTabType("onboarding")).toBeUndefined();
    expect(surfaceFromTabType(undefined)).toBeUndefined();
  });
});

describe("openSurfaceTab", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("home reuses an existing empty tab instead of opening a duplicate", () => {
    const homeTab = { type: "empty" as const, active: false, slotId: "s1", pinned: false };
    useTabs.getState().openNew(homeTab);
    const countBefore = useTabs.getState().tabs.length;

    openSurfaceTab("home");

    expect(useTabs.getState().tabs.length).toBe(countBefore);
    expect(useTabs.getState().currentTab?.type).toBe("empty");
  });

  test("home opens a fresh tab when none exists", () => {
    openSurfaceTab("home");
    expect(useTabs.getState().currentTab?.type).toBe("empty");
  });

  test("opens the meetings, calendar and action-items list surfaces", () => {
    openSurfaceTab("meetings");
    expect(useTabs.getState().currentTab?.type).toBe("meetings");

    openSurfaceTab("calendar");
    expect(useTabs.getState().currentTab?.type).toBe("calendar");

    openSurfaceTab("action-items");
    expect(useTabs.getState().currentTab?.type).toBe("action-items");
  });

  test("opens the library surfaces", () => {
    openSurfaceTab("templates");
    expect(useTabs.getState().currentTab?.type).toBe("templates");

    openSurfaceTab("snippets");
    expect(useTabs.getState().currentTab?.type).toBe("snippets");

    openSurfaceTab("contacts");
    expect(useTabs.getState().currentTab).toMatchObject({
      type: "contacts",
      state: { selected: null },
    });

    openSurfaceTab("trash");
    expect(useTabs.getState().currentTab?.type).toBe("trash");
  });

  test("opens settings", () => {
    openSurfaceTab("settings");
    expect(useTabs.getState().currentTab?.type).toBe("settings");
  });

  test("does not disturb an already-open note when opening another surface", () => {
    const note = createSessionTab({ id: "note-1" });
    useTabs.getState().openNew(note);

    openSurfaceTab("calendar");

    expect(
      useTabs
        .getState()
        .tabs.some((tab) => uniqueIdfromTab(tab) === uniqueIdfromTab(note)),
    ).toBe(true);
  });

  test("settings tab created elsewhere is still reachable via its own surface", () => {
    const settings = createSettingsTab();
    useTabs.getState().openNew(settings);
    expect(surfaceFromTabType(useTabs.getState().currentTab?.type)).toBe(
      "settings",
    );
  });
});

