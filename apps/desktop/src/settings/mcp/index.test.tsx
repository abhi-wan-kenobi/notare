import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  getStatus: vi.fn(),
  getBridgePath: vi.fn(),
  setSettings: vi.fn(),
  regenerateToken: vi.fn(),
}));

vi.mock("@hypr/plugin-mcp-server", () => ({
  commands: {
    getSettings: mocks.getSettings,
    getStatus: mocks.getStatus,
    getBridgePath: mocks.getBridgePath,
    setSettings: mocks.setSettings,
    regenerateToken: mocks.regenerateToken,
  },
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

describe("SettingsMcp", () => {
  beforeEach(() => {
    mocks.getSettings.mockResolvedValue({
      status: "ok",
      data: {
        enabled: true,
        port: 8787,
        bearer_token: "test-token-12345",
      },
    });
    mocks.getStatus.mockResolvedValue({
      status: "ok",
      data: {
        running: true,
        port: 8787,
        error: null,
      },
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("shows absolute sidecar path in Claude Desktop snippet when bridge binary is found", async () => {
    const sidecarPath = "/Applications/Notare.app/Contents/MacOS/notare-mcp";
    mocks.getBridgePath.mockResolvedValue({
      status: "ok",
      data: sidecarPath,
    });

    renderPage();

    await waitFor(() => {
      expect(
        screen.getByText("Claude Desktop (stdio bridge: notare-mcp)"),
      ).toBeDefined();
    });

    const claudePre = screen.getByText(new RegExp(sidecarPath));
    expect(claudePre).toBeDefined();
    expect(claudePre.textContent).toContain(`"command": "${sidecarPath}"`);
    expect(screen.queryByText(/The bridge binary wasn't found/)).toBeNull();
  });

  it("falls back to 'notare-mcp' with visible notice when sidecar is not found (dev builds)", async () => {
    mocks.getBridgePath.mockResolvedValue({
      status: "ok",
      data: null,
    });

    renderPage();

    await waitFor(() => {
      expect(screen.getByText(/The bridge binary wasn't found/)).toBeDefined();
    });

    const claudePre = screen.getByText(/"command": "notare-mcp"/);
    expect(claudePre).toBeDefined();
  });

  it("shows HTTP client snippet containing port and bearer token", async () => {
    mocks.getBridgePath.mockResolvedValue({
      status: "ok",
      data: null,
    });

    renderPage();

    await waitFor(() => {
      expect(
        screen.getByText(/HTTP clients \(omp, Cursor, Claude Code\)/),
      ).toBeDefined();
    });

    const httpPre = screen.getByText(
      new RegExp('"url": "http://127.0.0.1:8787"'),
    );
    expect(httpPre).toBeDefined();
    expect(httpPre.textContent).toContain(
      '"Authorization": "Bearer test-token-12345"',
    );
    expect(httpPre.textContent).toContain('"notare": {');
  });
});
