import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  setSettings: vi.fn(),
  regenerateToken: vi.fn(),
  getStatus: vi.fn(),
  writeText: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@hypr/plugin-mcp-server", () => ({
  commands: {
    getSettings: mocks.getSettings,
    setSettings: mocks.setSettings,
    regenerateToken: mocks.regenerateToken,
    getStatus: mocks.getStatus,
  },
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.writeText,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError, success: vi.fn() },
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string, ...values: unknown[]) => {
      if (typeof input === "string") {
        return input;
      }
      return (input as readonly string[]).reduce(
        (message: string, part: string, index: number) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    },
  }),
}));

import { SettingsMcp } from "./index";

const STORED = { enabled: false, port: 8787, bearer_token: "tok-old" };

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsMcp />
    </QueryClientProvider>,
  );
}

async function portInput() {
  return (await screen.findByRole("spinbutton")) as HTMLInputElement;
}

describe("SettingsMcp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getSettings.mockResolvedValue({ status: "ok", data: STORED });
    mocks.getStatus.mockResolvedValue({
      status: "ok",
      data: { running: false, port: 8787, error: null },
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a failed settings read inline instead of throwing out of the section", async () => {
    mocks.getSettings.mockResolvedValue({
      status: "error",
      error: "keychain is locked",
    });

    renderPage();

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("keychain is locked");
    expect(screen.getByText("Model Context Protocol (MCP)")).toBeTruthy();
  });

  it.each([
    ["an emptied field", ""],
    ["a privileged port", "80"],
    ["a port above the valid range", "70000"],
  ])("does not save %s and restores the stored port", async (_label, typed) => {
    renderPage();

    const input = await portInput();
    fireEvent.change(input, { target: { value: typed } });
    fireEvent.blur(input);

    expect(input.value).toBe("8787");
    expect(screen.getByRole("alert").textContent).toContain(
      "from 1024 to 65535",
    );
    expect(mocks.setSettings).not.toHaveBeenCalled();
  });

  it("saves a port inside the valid range", async () => {
    mocks.setSettings.mockResolvedValue({
      status: "ok",
      data: { ...STORED, port: 9000 },
    });

    renderPage();

    const input = await portInput();
    fireEvent.change(input, { target: { value: "9000" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(mocks.setSettings).toHaveBeenCalledWith({
        enabled: false,
        port: 9000,
      }),
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("turns the switch back off and says why when enabling fails", async () => {
    mocks.setSettings.mockResolvedValue({
      status: "error",
      error: "address already in use",
    });

    renderPage();

    const toggle = await screen.findByRole("switch");
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "address already in use",
      ),
    );
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });

  it("puts the stored port back when saving a new port fails", async () => {
    mocks.setSettings.mockResolvedValue({
      status: "error",
      error: "address already in use",
    });

    renderPage();

    const input = await portInput();
    fireEvent.change(input, { target: { value: "9000" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "address already in use",
      ),
    );
    expect(input.value).toBe("8787");
  });

  it("asks before regenerating the token, and regenerates only once confirmed", async () => {
    mocks.regenerateToken.mockResolvedValue({ status: "ok", data: "tok-new" });

    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Regenerate" }));

    const dialog = screen.getByRole("dialog");
    expect(
      within(dialog).getByText("Regenerate the bearer token?"),
    ).toBeTruthy();
    expect(mocks.regenerateToken).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Regenerate" }));

    await waitFor(() => expect(mocks.regenerateToken).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("leaves the token alone when the regenerate prompt is cancelled", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Regenerate" }));
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Cancel",
      }),
    );

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(mocks.regenerateToken).not.toHaveBeenCalled();
  });

  it("copies the token and marks it copied", async () => {
    mocks.writeText.mockResolvedValue(undefined);

    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Copy" }));

    await screen.findByRole("button", { name: "Copied" });
    expect(mocks.writeText).toHaveBeenCalledWith("tok-old");
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("reports a failed copy instead of claiming the token was copied", async () => {
    // jsdom has no navigator.clipboard, so the fallback fails as well.
    mocks.writeText.mockRejectedValue(new Error("clipboard unavailable"));

    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Copy" }));

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Couldn't copy to clipboard",
      ),
    );
    expect(screen.queryByRole("button", { name: "Copied" })).toBeNull();
  });
});
