import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("virtual:changelog", () => ({
  latestVersion: "0.6.7",
  latestContent:
    '---\ndate: "2026-11-04"\nsummary: "Bundled."\n---\n\n## Bundled\n',
}));

import { useChangelogContent } from "./data";

describe("useChangelogContent", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("serves the bundled notes for the current version without a network call", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useChangelogContent("0.6.7"));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.content).toContain("## Bundled");
    expect(result.current.date).toBe("2026-11-04");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("fetches notes for an older version from GitHub", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      text: async () =>
        '---\ndate: "2026-11-03"\nsummary: "Old."\n---\n\n## Old\n',
    });
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useChangelogContent("0.6.6"));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(fetchMock).toHaveBeenCalledWith(
      "https://raw.githubusercontent.com/abhi-wan-kenobi/notare/main/packages/changelog/content/0.6.6.md",
    );
    expect(result.current.content).toContain("## Old");
  });

  it.each([
    ["a 404", () => vi.fn().mockResolvedValue({ ok: false })],
    ["a network error", () => vi.fn().mockRejectedValue(new Error("offline"))],
  ])(
    "resolves to null content on %s so the page can show a fallback",
    async (_, make) => {
      vi.stubGlobal("fetch", make());

      const { result } = renderHook(() => useChangelogContent("0.5.0"));

      await waitFor(() => expect(result.current.loading).toBe(false));
      expect(result.current.content).toBeNull();
      expect(result.current.date).toBeNull();
    },
  );
});
