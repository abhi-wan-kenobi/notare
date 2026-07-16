import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  vaultBase: vi.fn(),
  obsidianVaults: vi.fn(),
  isEmptyOrMissingDir: vi.fn(),
  moveVault: vi.fn(),
  copyVault: vi.fn(),
  setVaultBase: vi.fn(),
  openPath: vi.fn(),
  homeDir: vi.fn(),
  selectFolder: vi.fn(),
  message: vi.fn(),
  scheduleAutomaticRelaunch: vi.fn(),
}));

vi.mock("@hypr/plugin-settings", () => ({
  commands: {
    vaultBase: mocks.vaultBase,
    obsidianVaults: mocks.obsidianVaults,
    isEmptyOrMissingDir: mocks.isEmptyOrMissingDir,
    moveVault: mocks.moveVault,
    copyVault: mocks.copyVault,
    setVaultBase: mocks.setVaultBase,
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openPath: mocks.openPath },
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: mocks.homeDir,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: mocks.message,
  open: mocks.selectFolder,
}));

vi.mock("~/shared/relaunch", () => ({
  scheduleAutomaticRelaunch: mocks.scheduleAutomaticRelaunch,
}));

import {
  NotesLocationSection,
  changeNotesLocation,
  resolveMigrationStrategy,
} from "./notes-location";

const CURRENT = "/home/user/.local/share/notare";
const TARGET = "/home/user/Documents/MyVault";

function renderSection() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <NotesLocationSection />
    </QueryClientProvider>,
  );
}

describe("resolveMigrationStrategy", () => {
  it("maps intent and destination state to a strategy", () => {
    expect(resolveMigrationStrategy(false, true)).toBe("switch");
    expect(resolveMigrationStrategy(false, false)).toBe("switch");
    expect(resolveMigrationStrategy(true, true)).toBe("move");
    expect(resolveMigrationStrategy(true, false)).toBe("copy");
  });
});

describe("changeNotesLocation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isEmptyOrMissingDir.mockResolvedValue({ status: "ok", data: true });
    mocks.moveVault.mockResolvedValue({ status: "ok", data: null });
    mocks.copyVault.mockResolvedValue({ status: "ok", data: null });
    mocks.setVaultBase.mockResolvedValue({ status: "ok", data: null });
  });

  it("moves the vault when migrating into an empty folder", async () => {
    await expect(changeNotesLocation(TARGET, true)).resolves.toBe("move");

    expect(mocks.moveVault).toHaveBeenCalledWith(TARGET);
    expect(mocks.copyVault).not.toHaveBeenCalled();
    expect(mocks.setVaultBase).not.toHaveBeenCalled();
  });

  it("copies into a non-empty folder and re-points the app", async () => {
    mocks.isEmptyOrMissingDir.mockResolvedValue({ status: "ok", data: false });

    await expect(changeNotesLocation(TARGET, true)).resolves.toBe("copy");

    expect(mocks.moveVault).not.toHaveBeenCalled();
    expect(mocks.copyVault).toHaveBeenCalledWith(TARGET);
    expect(mocks.setVaultBase).toHaveBeenCalledWith(TARGET);
  });

  it("only re-points the app when not migrating", async () => {
    await expect(changeNotesLocation(TARGET, false)).resolves.toBe("switch");

    expect(mocks.isEmptyOrMissingDir).not.toHaveBeenCalled();
    expect(mocks.moveVault).not.toHaveBeenCalled();
    expect(mocks.copyVault).not.toHaveBeenCalled();
    expect(mocks.setVaultBase).toHaveBeenCalledWith(TARGET);
  });

  it("propagates move errors", async () => {
    mocks.moveVault.mockResolvedValue({
      status: "error",
      error: "vault_base_is_not_empty",
    });

    await expect(changeNotesLocation(TARGET, true)).rejects.toThrow(
      "vault_base_is_not_empty",
    );
  });
});

describe("NotesLocationSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.homeDir.mockResolvedValue("/home/user");
    mocks.vaultBase.mockResolvedValue({ status: "ok", data: CURRENT });
    mocks.obsidianVaults.mockResolvedValue({
      status: "ok",
      data: [{ path: TARGET }],
    });
    mocks.isEmptyOrMissingDir.mockResolvedValue({ status: "ok", data: true });
    mocks.moveVault.mockResolvedValue({ status: "ok", data: null });
    mocks.copyVault.mockResolvedValue({ status: "ok", data: null });
    mocks.setVaultBase.mockResolvedValue({ status: "ok", data: null });
    mocks.message.mockResolvedValue(undefined);
    mocks.scheduleAutomaticRelaunch.mockResolvedValue("scheduled");
    mocks.selectFolder.mockResolvedValue(null);
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the current notes folder", async () => {
    renderSection();

    await waitFor(() => {
      expect(screen.getByText("~/.local/share/notare")).toBeTruthy();
    });
  });

  it("offers migration when a detected vault is selected, then moves", async () => {
    renderSection();

    const vaultButton = await screen.findByText("~/Documents/MyVault");
    fireEvent.click(vaultButton);

    const moveButton = await screen.findByText("Move notes");
    fireEvent.click(moveButton);

    await waitFor(() => {
      expect(mocks.moveVault).toHaveBeenCalledWith(TARGET);
    });
    expect(mocks.scheduleAutomaticRelaunch).toHaveBeenCalled();
  });

  it("switches without touching files when declining migration", async () => {
    renderSection();

    const vaultButton = await screen.findByText("~/Documents/MyVault");
    fireEvent.click(vaultButton);

    const dontMoveButton = await screen.findByText("Don't move");
    fireEvent.click(dontMoveButton);

    await waitFor(() => {
      expect(mocks.setVaultBase).toHaveBeenCalledWith(TARGET);
    });
    expect(mocks.moveVault).not.toHaveBeenCalled();
    expect(mocks.copyVault).not.toHaveBeenCalled();
    expect(mocks.scheduleAutomaticRelaunch).toHaveBeenCalled();
  });

  it("opens the folder picker on Change", async () => {
    mocks.selectFolder.mockResolvedValue(TARGET);
    renderSection();

    const changeButton = await screen.findByText("Change");
    fireEvent.click(changeButton);

    await screen.findByText("Move existing notes?");
    expect(mocks.selectFolder).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, multiple: false }),
    );
  });
});
