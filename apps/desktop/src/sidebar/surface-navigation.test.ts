import { beforeEach, describe, expect, test } from "vitest";

import {
  goToNotesSurface,
  openSurfaceTab,
  surfaceFromTabType,
  type SurfaceId,
} from "./surface-navigation";

import { useTabs } from "~/store/zustand/tabs";
import {
  createSessionTab,
  createSettingsTab,
  resetTabsStore,
} from "~/store/zustand/tabs/test-utils";

type SpecialSurface = Exclude<SurfaceId, "notes">;

const SPECIAL_SURFACES: SpecialSurface[] = [
  "calendar",
  "contacts",
  "templates",
  "settings",
];

const openNoteTab = (id: string) => {
  const note = createSessionTab({ id });
  useTabs.getState().openNew(note);
  return note;
};

const currentSurface = () =>
  surfaceFromTabType(useTabs.getState().currentTab?.type);

const countTabsOfSurface = (surface: SpecialSurface) =>
  useTabs.getState().tabs.filter((tab) => tab.type === surface).length;

describe("surface rail navigation", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("goToNotesSurface is a no-op on a notes surface", () => {
    openNoteTab("note-1");

    goToNotesSurface();

    expect(useTabs.getState().currentTab).toMatchObject({
      type: "sessions",
      id: "note-1",
    });
    expect(useTabs.getState().tabs).toHaveLength(1);
  });

  describe.each(SPECIAL_SURFACES)("notes → %s → notes", (surface) => {
    test("returns to the origin note", () => {
      openNoteTab("note-1");

      openSurfaceTab(surface);
      expect(currentSurface()).toBe(surface);

      goToNotesSurface();

      expect(useTabs.getState().currentTab).toMatchObject({
        type: "sessions",
        id: "note-1",
      });
    });
  });

  const surfacePairs = SPECIAL_SURFACES.flatMap((from) =>
    SPECIAL_SURFACES.filter((to) => to !== from).map(
      (to) => [from, to] as const,
    ),
  );

  describe.each(surfacePairs)("notes → %s → %s → notes", (first, second) => {
    test("rail shows each surface and notes returns to the origin note", () => {
      openNoteTab("note-1");

      openSurfaceTab(first);
      expect(currentSurface()).toBe(first);

      openSurfaceTab(second);
      expect(currentSurface()).toBe(second);

      // The Notes action must land on the note, never on the first special
      // surface (the reported wrong-surface bug).
      goToNotesSurface();
      expect(useTabs.getState().currentTab).toMatchObject({
        type: "sessions",
        id: "note-1",
      });

      expect(countTabsOfSurface(first)).toBe(1);
      expect(countTabsOfSurface(second)).toBe(1);
    });
  });

  test("three-surface chain still returns to the origin note", () => {
    openNoteTab("note-1");

    openSurfaceTab("settings");
    openSurfaceTab("calendar");
    openSurfaceTab("templates");
    expect(currentSurface()).toBe("templates");

    goToNotesSurface();

    expect(useTabs.getState().currentTab).toMatchObject({
      type: "sessions",
      id: "note-1",
    });
  });

  test("revisiting a surface in a chain reuses its tab", () => {
    openNoteTab("note-1");

    openSurfaceTab("settings");
    openSurfaceTab("calendar");
    openSurfaceTab("settings");

    expect(currentSurface()).toBe("settings");
    expect(countTabsOfSurface("settings")).toBe(1);
    expect(countTabsOfSurface("calendar")).toBe(1);

    goToNotesSurface();
    expect(useTabs.getState().currentTab).toMatchObject({
      type: "sessions",
      id: "note-1",
    });
  });

  test("origin follows the most recently visited note", () => {
    const noteA = openNoteTab("note-a");
    openSurfaceTab("settings");

    const noteB = openNoteTab("note-b");
    openSurfaceTab("settings");
    expect(countTabsOfSurface("settings")).toBe(1);

    goToNotesSurface();
    expect(useTabs.getState().currentTab).toMatchObject({
      type: "sessions",
      id: noteB.id,
    });
    expect(
      useTabs
        .getState()
        .tabs.some((tab) => tab.type === "sessions" && tab.id === noteA.id),
    ).toBe(true);
  });

  test("chain with no note origin falls back to a home tab", () => {
    const settings = createSettingsTab({ active: true });
    useTabs.getState().openNew(settings);

    openSurfaceTab("calendar");
    expect(currentSurface()).toBe("calendar");

    goToNotesSurface();

    expect(useTabs.getState().currentTab).toMatchObject({ type: "empty" });
  });

  test("stale return origin (slot now holds another tab) falls back to home", () => {
    openNoteTab("note-1");
    openSurfaceTab("settings");

    // Simulate the origin slot being reused by a different note.
    useTabs.setState((state) => ({
      tabs: state.tabs.map((tab) =>
        tab.type === "sessions" ? { ...tab, id: "note-2" } : tab,
      ),
    }));

    goToNotesSurface();

    expect(useTabs.getState().currentTab).toMatchObject({ type: "empty" });
  });

  test("closing the origin note clears the surface's return origin", () => {
    const note = openNoteTab("note-1");
    openSurfaceTab("settings");

    const noteTab = useTabs
      .getState()
      .tabs.find((tab) => tab.type === "sessions" && tab.id === note.id);
    expect(noteTab).toBeTruthy();
    useTabs.getState().close(noteTab!);

    goToNotesSurface();

    expect(useTabs.getState().currentTab).toMatchObject({ type: "empty" });
  });
});
