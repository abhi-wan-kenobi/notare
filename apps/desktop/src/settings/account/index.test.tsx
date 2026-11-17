import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  globalBase: vi.fn(),
  vaultBase: vi.fn(),
  openPath: vi.fn(),
  message: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: vi.fn().mockResolvedValue("/home/test"),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ message: mocks.message }));

vi.mock("@hypr/plugin-settings", () => ({
  commands: { globalBase: mocks.globalBase, vaultBase: mocks.vaultBase },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openPath: mocks.openPath },
}));

vi.mock("~/settings/general/storage", () => ({
  StorageSettingsView: () => <div>Storage settings</div>,
}));

import { AccountSettingsView } from "./index";

function renderSettings() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <AccountSettingsView />
    </QueryClientProvider>,
  );
}

describe("AccountSettingsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.globalBase.mockResolvedValue({
      status: "ok",
      data: "/home/test/.local/share/notare",
    });
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/home/test/Documents/My notes",
    });
    mocks.openPath.mockResolvedValue({ status: "ok", data: null });
  });

  afterEach(cleanup);

  it("opens the notes folder without promising a bulk export or requiring an account", async () => {
    renderSettings();

    expect(screen.getByText("No Notare account required")).toBeTruthy();
    expect(screen.queryByText("Export notes")).toBeNull();
    expect(screen.queryByText(/Markdown or JSON/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Export" })).toBeNull();
    expect(
      screen.getByText(/Use Export in an individual meeting/),
    ).toBeTruthy();

    const button = screen.getByRole<HTMLButtonElement>("button", {
      name: "Open notes folder",
    });
    await waitFor(() => expect(button.disabled).toBe(false));
    fireEvent.click(button);

    expect(mocks.openPath).toHaveBeenCalledOnce();
    expect(mocks.openPath).toHaveBeenCalledWith(
      "/home/test/Documents/My notes",
      null,
    );
  });

  it("reveals the application data directory rather than the separate notes folder", async () => {
    renderSettings();

    const button = screen.getByRole<HTMLButtonElement>("button", {
      name: "Reveal in folder",
    });
    await waitFor(() => expect(button.disabled).toBe(false));
    fireEvent.click(button);

    expect(mocks.globalBase).toHaveBeenCalledOnce();
    expect(mocks.openPath).toHaveBeenCalledOnce();
    expect(mocks.openPath).toHaveBeenCalledWith(
      "/home/test/.local/share/notare",
      null,
    );
  });

  it("shows non-destructive reset help with real paths and accurate deletion boundaries", async () => {
    renderSettings();
    await screen.findByText("~/.local/share/notare");

    fireEvent.click(screen.getByRole("button", { name: "Reset instructions" }));
    const dialog = screen.getByRole("dialog");
    expect(
      within(dialog).getByRole("heading", { name: "Reset instructions" }),
    ).toBeTruthy();
    expect(
      within(dialog).getByText(/Notare will not delete any files/),
    ).toBeTruthy();
    expect(
      within(dialog).getByText("/home/test/.local/share/notare"),
    ).toBeTruthy();
    expect(
      within(dialog).getByText("/home/test/Documents/My notes"),
    ).toBeTruthy();
    expect(within(dialog).getByText(/Removing database files/)).toBeTruthy();
    expect(
      within(dialog).getByText(/not current database preferences/),
    ).toBeTruthy();
    expect(
      within(dialog).getByText(
        /does not revoke credentials or clear system keychain entries/,
      ),
    ).toBeTruthy();
    expect(
      within(dialog).getByText(
        /Recordings, attachments, and Markdown files remain/,
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Delete all local data?")).toBeNull();
    expect(screen.queryByRole("button", { name: "Confirm reset" })).toBeNull();
    expect(mocks.openPath).not.toHaveBeenCalled();
    expect(mocks.message).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.message).not.toHaveBeenCalled();
  });

  it("does not reveal a guessed folder when the real path cannot be loaded", async () => {
    mocks.globalBase.mockResolvedValue({
      status: "error",
      error: "Unavailable",
    });
    mocks.vaultBase.mockResolvedValue({
      status: "error",
      error: "Unavailable",
    });
    renderSettings();

    const dataButton = screen.getByRole<HTMLButtonElement>("button", {
      name: "Reveal in folder",
    });
    const notesButton = screen.getByRole<HTMLButtonElement>("button", {
      name: "Open notes folder",
    });
    expect(dataButton.disabled).toBe(true);
    expect(notesButton.disabled).toBe(true);
    fireEvent.click(dataButton);
    fireEvent.click(notesButton);
    fireEvent.click(screen.getByRole("button", { name: "Reset instructions" }));
    expect(
      await screen.findByText("Could not load the data directory."),
    ).toBeTruthy();
    expect(
      await screen.findByText("Could not load the notes folder."),
    ).toBeTruthy();
    expect(mocks.openPath).not.toHaveBeenCalled();
    expect(mocks.message).not.toHaveBeenCalled();
  });
});
