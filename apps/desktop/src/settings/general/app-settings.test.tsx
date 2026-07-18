import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  updateControl: {
    status: null as string | null,
    version: null as string | null,
    progress: null as number | null,
    errorMessage: null as string | null,
    downloadStarting: false,
    installing: false,
    downloadUpdate: vi.fn(),
    installUpdate: vi.fn(),
  },
  refetchQueries: vi.fn(async () => undefined),
  getVersion: vi.fn(async () => "0.2.0"),
  configValues: { mic_denoise: false } as Record<string, unknown>,
  setSettingValue: vi.fn(),
}));

vi.mock("~/main/update-banner", () => ({
  UPDATE_CHECK_QUERY_KEY: ["updater2", "check"],
  useDesktopUpdateControl: () => mocks.updateControl,
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ refetchQueries: mocks.refetchQueries }),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: mocks.getVersion,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setSettingValue,
}));

import { AppSettingsView } from "./app-settings";

function setting(value = true) {
  return {
    value,
    onChange: vi.fn(),
  };
}

function renderAppSettings({ floatingBar = true } = {}) {
  return render(
    <AppSettingsView
      autostart={setting()}
      autoStartScheduledMeetings={setting()}
      autoStopMeetings={setting()}
      floatingBar={setting(floatingBar)}
      showAppInDock={setting()}
      showTrayIcon={setting()}
      telemetryConsent={setting()}
    />,
  );
}

describe("AppSettingsView", () => {
  beforeEach(() => {
    mocks.updateControl.status = null;
    mocks.updateControl.version = null;
    mocks.updateControl.progress = null;
    mocks.updateControl.errorMessage = null;
    mocks.updateControl.downloadStarting = false;
    mocks.updateControl.installing = false;
    mocks.updateControl.downloadUpdate.mockReset();
    mocks.updateControl.installUpdate.mockReset();
    mocks.refetchQueries.mockClear();
    mocks.getVersion.mockClear();
    mocks.setSettingValue.mockReset();
    mocks.configValues.mic_denoise = false;
  });

  afterEach(() => {
    cleanup();
  });

  it("does not expose a separate live transcript overlay setting", () => {
    renderAppSettings();

    expect(screen.queryByText("Show live transcript overlay")).toBeNull();
  });

  it("keeps the floating bar setting available", () => {
    renderAppSettings({ floatingBar: false });

    expect(screen.getByText("Show floating bar")).toBeTruthy();
  });

  it("shows the mic noise suppression toggle bound to mic_denoise", () => {
    renderAppSettings();

    expect(
      screen.getByText("Microphone noise suppression (experimental)"),
    ).toBeTruthy();

    const toggle = screen.getByRole("switch", {
      name: "Microphone noise suppression (experimental)",
    });
    expect(toggle.getAttribute("aria-checked")).toBe("false");

    fireEvent.click(toggle);
    expect(mocks.setSettingValue).toHaveBeenCalledWith(true);
  });

  it("shows the current app version", async () => {
    renderAppSettings();

    await waitFor(() =>
      expect(screen.getByTestId("current-version").textContent).toBe(
        "Notare 0.2.0",
      ),
    );
  });

  it("checks for updates via the shared poll query", async () => {
    renderAppSettings();

    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));

    await waitFor(() =>
      expect(mocks.refetchQueries).toHaveBeenCalledWith({
        queryKey: ["updater2", "check"],
        exact: true,
      }),
    );
    // No update afterwards: the up-to-date note appears.
    expect(await screen.findByTestId("up-to-date")).toBeTruthy();
  });

  it("shows an available update with a download action", () => {
    mocks.updateControl.status = "available";
    mocks.updateControl.version = "0.3.0";

    renderAppSettings();

    expect(screen.getByTestId("update-state").textContent).toContain("0.3.0");

    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(mocks.updateControl.downloadUpdate).toHaveBeenCalledTimes(1);
  });

  it("shows download progress and disables the action while downloading", () => {
    mocks.updateControl.status = "downloading";
    mocks.updateControl.version = "0.3.0";
    mocks.updateControl.progress = 0.4;

    renderAppSettings();

    expect(screen.getByTestId("update-state").textContent).toContain("40%");
    const button = screen.getByRole("button", { name: "Downloading…" });
    expect(button.hasAttribute("disabled")).toBe(true);
  });

  it("offers restart when the update is ready", () => {
    mocks.updateControl.status = "ready";
    mocks.updateControl.version = "0.3.0";

    renderAppSettings();

    fireEvent.click(screen.getByRole("button", { name: "Restart to update" }));
    expect(mocks.updateControl.installUpdate).toHaveBeenCalledTimes(1);
  });

  it("surfaces a failed download with a retry action", () => {
    mocks.updateControl.status = "failed";
    mocks.updateControl.version = "0.3.0";
    mocks.updateControl.errorMessage = "Failed to download update.";

    renderAppSettings();
    expect(screen.getByTestId("update-state").textContent).toBe(
      "Failed to download update.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry download" }));
    expect(mocks.updateControl.downloadUpdate).toHaveBeenCalledTimes(1);
  });
});

describe("Meeting bar theme picker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.configValues = { mic_denoise: false } as Record<string, unknown>;
    mocks.setSettingValue.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the Notare / Classic picker under the floating bar setting", () => {
    renderAppSettings();

    const group = screen.getByTestId("meeting-bar-theme-group");
    expect(group).toBeTruthy();
    expect(group.textContent).toContain("Notare");
    expect(group.textContent).toContain("Classic");
  });

  it("defaults to Notare when meeting_bar_theme is unset", () => {
    renderAppSettings();

    expect(
      (screen.getByRole("radio", { name: /Notare/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("radio", { name: /Classic/ }) as HTMLInputElement)
        .checked,
    ).toBe(false);
  });

  it("marks Classic as selected when meeting_bar_theme is 'classic'", () => {
    mocks.configValues.meeting_bar_theme = "classic";

    renderAppSettings();

    expect(
      (screen.getByRole("radio", { name: /Classic/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("radio", { name: /Notare/ }) as HTMLInputElement)
        .checked,
    ).toBe(false);
  });

  it("writes 'classic' when the Classic radio is selected", () => {
    // Default Notare is checked, so Classic is unchecked and its click fires.
    renderAppSettings();

    fireEvent.click(screen.getByRole("radio", { name: /Classic/ }));
    expect(mocks.setSettingValue).toHaveBeenCalledWith("classic");
  });

  it("writes 'notare' when switching back from Classic", () => {
    // Classic is checked, so Notare is unchecked and its click fires.
    mocks.configValues.meeting_bar_theme = "classic";

    renderAppSettings();

    fireEvent.click(screen.getByRole("radio", { name: /Notare/ }));
    expect(mocks.setSettingValue).toHaveBeenLastCalledWith("notare");
  });
});
