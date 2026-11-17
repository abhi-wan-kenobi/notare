import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  selectFolder: vi.fn(),
  copyVault: vi.fn(),
  setVaultBase: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: vi.fn().mockResolvedValue("/home/user"),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: vi.fn(),
  open: mocks.selectFolder,
}));
vi.mock("@hypr/plugin-opener2", () => ({ commands: { openPath: vi.fn() } }));
vi.mock("@hypr/plugin-settings", () => ({
  commands: {
    vaultBase: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: "/home/user/notes" }),
    obsidianVaults: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    copyVault: mocks.copyVault,
    setVaultBase: mocks.setVaultBase,
  },
}));
vi.mock("~/shared/relaunch", () => ({
  scheduleAutomaticRelaunch: vi.fn().mockResolvedValue("deferred"),
}));

import { FolderLocationSection } from "./folder-location";

describe("onboarding storage location", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.selectFolder.mockResolvedValue("/mnt/full-disk");
    mocks.setVaultBase.mockResolvedValue({ status: "ok", data: null });
  });
  afterEach(cleanup);

  it("shows why changing the storage location failed", async () => {
    mocks.copyVault.mockResolvedValue({
      status: "error",
      error: "No space left on device",
    });
    render(
      <QueryClientProvider client={new QueryClient()}>
        <FolderLocationSection onContinue={vi.fn()} />
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Change" }));

    expect(await screen.findByText("No space left on device")).toBeTruthy();
    expect(mocks.setVaultBase).not.toHaveBeenCalled();
  });
});
