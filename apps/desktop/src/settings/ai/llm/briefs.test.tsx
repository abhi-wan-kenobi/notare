import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { LanguageModel } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BriefsSettings } from "./briefs";

import type { ResolveResult } from "~/services/llm-router";

const mocks = vi.hoisted(() => ({
  configValues: {} as Record<string, unknown>,
  setValues: vi.fn(),
  taskModel: {
    model: {} as unknown as LanguageModel,
    resolution: {
      status: "ok" as const,
      providerId: "ollama",
      modelId: "llama3",
    } as ResolveResult,
    target: null,
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: (key: string) => (val: unknown) =>
    mocks.setValues(key, val),
}));

vi.mock("~/services/llm-router", () => ({
  useTaskModel: () => mocks.taskModel,
}));

describe("BriefsSettings Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.configValues = {
      pre_meeting_briefs_enabled: true,
      pre_meeting_briefs_lead_time_hours: 12,
    };
    mocks.taskModel = {
      model: {} as unknown as LanguageModel,
      resolution: {
        status: "ok" as const,
        providerId: "ollama",
        modelId: "llama3",
      } as unknown as ResolveResult,
      target: null,
    };
  });

  afterEach(() => {
    cleanup();
  });

  it("renders pre-meeting briefs section with toggle and lead time", () => {
    render(<BriefsSettings />);

    expect(screen.getByText("Pre-Meeting Briefs")).toBeDefined();
    expect(screen.getByText("Generate pre-meeting briefs")).toBeDefined();
    expect(screen.getByRole("switch")).toBeDefined();
    expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe(
      "true",
    );
    expect(screen.getByText("Preparation window")).toBeDefined();
  });

  it("disables switch when no model is configured", () => {
    mocks.taskModel = {
      model: null as unknown as LanguageModel,
      resolution: { status: "unavailable" as const, reason: "no_provider" },
      target: null,
    };

    render(<BriefsSettings />);

    const toggle = screen.getByRole("switch");
    expect(toggle.hasAttribute("disabled")).toBe(true);
    expect(
      screen.getByText(/Configure a local or cloud model above/i),
    ).toBeDefined();
    expect(screen.queryByText("Preparation window")).toBeNull();
  });

  it("toggles briefs enabled setting", () => {
    render(<BriefsSettings />);

    const toggle = screen.getByRole("switch");
    fireEvent.click(toggle);

    expect(mocks.setValues).toHaveBeenCalledWith(
      "pre_meeting_briefs_enabled",
      false,
    );
  });
});
