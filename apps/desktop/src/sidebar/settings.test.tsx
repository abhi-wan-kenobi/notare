import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "settings", state: { tab: "general" } } as {
    type: "settings";
    state: { tab?: string };
  } | null,
  openNew: vi.fn(),
  updateSettingsTabState: vi.fn(),
}));

const lingui = vi.hoisted(() => {
  const t = (
    input: TemplateStringsArray | { message?: string } | string,
    ...values: unknown[]
  ) => {
    if (Array.isArray(input)) {
      return input.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    }

    if (typeof input === "string") {
      return input;
    }

    if ("message" in input) {
      return input.message ?? "";
    }

    return "";
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("./custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ title }: { title: ReactNode }) => <div>{title}</div>,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: mocks.currentTab,
      openNew: mocks.openNew,
      updateSettingsTabState: mocks.updateSettingsTabState,
    }),
}));

import { SettingsNav } from "./settings";

describe("SettingsNav", () => {
  beforeEach(() => {
    mocks.currentTab = { type: "settings", state: { tab: "general" } };
    mocks.openNew.mockClear();
    mocks.updateSettingsTabState.mockClear();
  });

  it("shows four settings hubs and updates the active hub", () => {
    render(<SettingsNav />);

    const hubs = [
      "General & System",
      "AI & Speech",
      "Integrations & Sync",
      "Account & Storage",
    ];
    for (const label of hubs) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
    expect(screen.getAllByRole("button")).toHaveLength(4);

    screen.getByRole("button", { name: "Integrations & Sync" }).click();
    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "integrations" },
    );
  });
});
