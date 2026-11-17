import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyticsEvent: vi.fn(),
  getOrCreateWelcomeSession: vi.fn(),
  flushAutomaticRelaunch: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@iconify-icon/react", () => ({ Icon: () => null }));
vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: mocks.analyticsEvent },
}));
vi.mock("@hypr/plugin-opener2", () => ({ commands: { openUrl: vi.fn() } }));
vi.mock("@hypr/plugin-sfx", () => ({
  commands: { stop: vi.fn().mockResolvedValue(undefined) },
}));
vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));
vi.mock("./welcome-note", () => ({
  getOrCreateWelcomeSession: mocks.getOrCreateWelcomeSession,
  setPendingWelcomeSession: vi.fn(),
}));
vi.mock("~/shared/relaunch", () => ({
  flushAutomaticRelaunch: mocks.flushAutomaticRelaunch,
}));
vi.mock("~/types/tauri.gen", () => ({
  commands: { setOnboardingNeeded: vi.fn().mockResolvedValue(undefined) },
}));

import { FinalSection } from "./final";

const openButton = () => screen.getByRole("button", { name: "Open Notare" });

describe("onboarding final step", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.analyticsEvent.mockResolvedValue(undefined);
    mocks.getOrCreateWelcomeSession.mockResolvedValue("welcome-session");
    mocks.flushAutomaticRelaunch.mockResolvedValue(false);
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("still opens the app when analytics fails", async () => {
    mocks.analyticsEvent.mockRejectedValue(new Error("analytics offline"));
    const onContinue = vi.fn();
    render(<FinalSection onContinue={onContinue} />);

    fireEvent.click(openButton());

    await waitFor(() =>
      expect(onContinue).toHaveBeenCalledWith("welcome-session"),
    );
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("still opens the app when the pending relaunch fails", async () => {
    mocks.flushAutomaticRelaunch.mockRejectedValue(new Error("no relaunch"));
    const onContinue = vi.fn();
    render(<FinalSection onContinue={onContinue} />);

    fireEvent.click(openButton());

    await waitFor(() =>
      expect(onContinue).toHaveBeenCalledWith("welcome-session"),
    );
  });

  it("shows an error and lets the user retry when the welcome note fails", async () => {
    mocks.getOrCreateWelcomeSession.mockRejectedValueOnce(new Error("db down"));
    const onContinue = vi.fn();
    render(<FinalSection onContinue={onContinue} />);

    fireEvent.click(openButton());

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
    expect(onContinue).not.toHaveBeenCalled();
    await waitFor(() => expect(openButton()).toHaveProperty("disabled", false));

    fireEvent.click(openButton());

    await waitFor(() =>
      expect(onContinue).toHaveBeenCalledWith("welcome-session"),
    );
  });

  it("runs once when the button is clicked twice", async () => {
    const onContinue = vi.fn();
    render(<FinalSection onContinue={onContinue} />);

    fireEvent.click(openButton());
    expect(openButton()).toHaveProperty("disabled", true);
    fireEvent.click(openButton());

    await waitFor(() => expect(onContinue).toHaveBeenCalledTimes(1));
    expect(mocks.getOrCreateWelcomeSession).toHaveBeenCalledTimes(1);
  });
});
