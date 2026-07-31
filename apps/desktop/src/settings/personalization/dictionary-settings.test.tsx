import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

// jsdom has no ResizeObserver; Radix's Switch (used for the per-entry
// case-sensitive toggle) mounts one via @radix-ui/react-use-size regardless
// of whether anything actually needs the reported size in this test env.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver =
    ResizeObserverStub as unknown as typeof ResizeObserver;
}

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
    t: (
      input: TemplateStringsArray | { message?: string } | string,
      ...values: unknown[]
    ) => {
      if (typeof input === "string") {
        return input;
      }

      if (Array.isArray(input)) {
        return (input as readonly string[]).reduce(
          (message: string, part: string, index: number) =>
            `${message}${part}${index < values.length ? String(values[index]) : ""}`,
          "",
        );
      }

      return (input as { message?: string }).message ?? "";
    },
  }),
}));

const mocks = vi.hoisted(() => ({
  selectFile: vi.fn(),
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
  revealItemInDir: vi.fn(),
  downloadDir: vi.fn(),
  join: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: mocks.selectFile,
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: mocks.downloadDir,
  join: mocks.join,
}));

vi.mock("@hypr/plugin-fs2", () => ({
  commands: {
    readTextFile: mocks.readTextFile,
    writeTextFile: mocks.writeTextFile,
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    revealItemInDir: mocks.revealItemInDir,
  },
}));

// `~/dictation/dictionary` is the engine module another agent is landing
// concurrently in this branch. We mock it here with a small stand-in that
// implements the agreed contract (see the module docstring in
// dictionary-settings.tsx) so this component's tests don't depend on that
// work landing first.
vi.mock("~/dictation/dictionary", () => {
  type DictionaryMapping = {
    wrong: string;
    right: string;
    caseSensitive: boolean;
  };
  type DictionaryEntry = string | DictionaryMapping;

  // Mirrors the real module's contract: tolerant of garbage, returns []
  // instead of throwing (the component relies on that, no wrapper).
  function parseDictionaryEntries(raw: string): DictionaryEntry[] {
    try {
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }

  function serializeDictionaryEntries(entries: DictionaryEntry[]): string {
    return JSON.stringify(entries);
  }

  function importDictionaryText(text: string): DictionaryEntry[] {
    return text
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line): DictionaryEntry => {
        const match = line.match(/^(.*?)\s*=>\s*(.*?)(\s*\[cs\])?$/);
        if (!match) return line;
        const [, wrong, right, cs] = match;
        return { wrong, right, caseSensitive: Boolean(cs) };
      });
  }

  function exportDictionaryText(entries: DictionaryEntry[]): string {
    return entries
      .map((entry) =>
        typeof entry === "string"
          ? entry
          : `${entry.wrong} => ${entry.right}${entry.caseSensitive ? " [cs]" : ""}`,
      )
      .join("\n");
  }

  return {
    parseDictionaryEntries,
    serializeDictionaryEntries,
    importDictionaryText,
    exportDictionaryText,
  };
});

import { DictionarySettings, mergeDictionaryEntries } from "./dictionary-settings";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("DictionarySettings", () => {
  it("shows an empty state and disables export when there are no entries", () => {
    render(<DictionarySettings raw="[]" onSave={vi.fn()} />);

    expect(screen.getByText("No dictionary entries yet.")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: /Export/ }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("renders mixed legacy flat terms and mappings distinctly", () => {
    const raw = JSON.stringify([
      "Notare",
      { wrong: "far eye", right: "FarEye", caseSensitive: true },
    ]);
    render(<DictionarySettings raw={raw} onSave={vi.fn()} />);

    expect(screen.getByText("Notare")).toBeTruthy();
    expect(screen.getByText("far eye")).toBeTruthy();
    expect(screen.getByText("FarEye")).toBeTruthy();
    expect(screen.getByText("Aa")).toBeTruthy(); // case-sensitive badge
  });

  it("adds a mapping entry with the case-sensitive flag", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings raw="[]" onSave={onSave} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Wrong text, or names/jargon to prefer" }),
      { target: { value: "far eye" } },
    );
    fireEvent.change(
      screen.getByRole("textbox", { name: "Replace with (optional)" }),
      { target: { value: "FarEye" } },
    );
    fireEvent.click(screen.getByRole("switch"));

    const addButton = screen.getByRole("button", { name: "Add" });
    await waitFor(() => expect((addButton as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(addButton);

    expect(onSave).toHaveBeenCalledWith(
      JSON.stringify([{ wrong: "far eye", right: "FarEye", caseSensitive: true }]),
    );
  });

  it("adds a batch of flat terms when no replacement is given", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={onSave} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Wrong text, or names/jargon to prefer" }),
      { target: { value: " FastConformer, Parakeet TDT " } },
    );

    const addButton = screen.getByRole("button", { name: "Add" });
    await waitFor(() => expect((addButton as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(addButton);

    expect(onSave).toHaveBeenCalledWith(
      JSON.stringify(["Notare", "FastConformer", "Parakeet TDT"]),
    );
  });

  it("warns on duplicate (case-insensitive) and disables add", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={onSave} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Wrong text, or names/jargon to prefer" }),
      { target: { value: "notare" } },
    );

    await waitFor(() =>
      expect(screen.getByText(/already in your dictionary/)).toBeTruthy(),
    );
    const addButton = screen.getByRole("button", { name: "Add" }) as HTMLButtonElement;
    expect(addButton.disabled).toBe(true);

    fireEvent.click(addButton);
    expect(onSave).not.toHaveBeenCalled();
  });

  it("rejects empty-field submission (Add stays disabled)", () => {
    render(<DictionarySettings raw="[]" onSave={vi.fn()} />);
    expect(
      (screen.getByRole("button", { name: "Add" }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("edits an entry in place, converting a term into a mapping", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={onSave} />);

    fireEvent.click(screen.getByRole("button", { name: /Edit Notare/ }));

    const rightInput = screen.getByRole("textbox", {
      name: "Edit replacement text",
    });
    fireEvent.change(rightInput, { target: { value: "Notare Inc." } });

    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith(
      JSON.stringify([{ wrong: "Notare", right: "Notare Inc.", caseSensitive: false }]),
    );
  });

  it("blocks saving an edit that collides with another entry", () => {
    const onSave = vi.fn();
    render(
      <DictionarySettings
        raw={JSON.stringify(["Notare", "FastConformer"])}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Edit Notare/ }));
    const wrongInput = screen.getByRole("textbox", {
      name: "Edit wrong text",
    });
    fireEvent.change(wrongInput, { target: { value: "FastConformer" } });

    expect(
      (screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).not.toHaveBeenCalled();
  });

  it("cancels an in-place edit without saving", () => {
    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /Edit Notare/ }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.getByText("Notare")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  });

  it("removes an entry", () => {
    const onSave = vi.fn();
    render(
      <DictionarySettings
        raw={JSON.stringify(["Notare", "Parakeet TDT"])}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Remove Notare/ }));

    expect(onSave).toHaveBeenCalledWith(JSON.stringify(["Parakeet TDT"]));
  });

  it("imports a text file, merging and deduping against existing entries", async () => {
    const onSave = vi.fn();
    mocks.selectFile.mockResolvedValue("/tmp/dictionary.txt");
    mocks.readTextFile.mockResolvedValue({
      status: "ok",
      data: [
        "Notare", // duplicate of existing -> no-op (identical)
        "FastConformer", // new flat term
        "far eye => FarEye Corp", // updates the existing mapping's right side
      ].join("\n"),
    });

    const raw = JSON.stringify([
      "Notare",
      { wrong: "far eye", right: "FarEye", caseSensitive: false },
    ]);
    render(<DictionarySettings raw={raw} onSave={onSave} />);

    fireEvent.click(screen.getByRole("button", { name: "Import…" }));

    await waitFor(() => expect(onSave).toHaveBeenCalled());

    expect(onSave).toHaveBeenCalledWith(
      JSON.stringify([
        "Notare",
        { wrong: "far eye", right: "FarEye Corp", caseSensitive: false },
        "FastConformer",
      ]),
    );
    await waitFor(() =>
      expect(
        screen.getByText(/Import finished: 1 added, 1 updated\./),
      ).toBeTruthy(),
    );
  });

  it("shows an import error instead of crashing when the file can't be read", async () => {
    mocks.selectFile.mockResolvedValue("/tmp/dictionary.txt");
    mocks.readTextFile.mockResolvedValue({
      status: "error",
      error: "permission denied",
    });

    render(<DictionarySettings raw="[]" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Import…" }));

    await waitFor(() =>
      expect(screen.getByText(/Import failed: permission denied/)).toBeTruthy(),
    );
  });

  it("does nothing when the import dialog is cancelled", async () => {
    const onSave = vi.fn();
    mocks.selectFile.mockResolvedValue(null);

    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={onSave} />);
    fireEvent.click(screen.getByRole("button", { name: "Import…" }));

    await waitFor(() => expect(mocks.selectFile).toHaveBeenCalled());
    expect(mocks.readTextFile).not.toHaveBeenCalled();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("exports the engine's serialized text and reveals the saved file", async () => {
    mocks.downloadDir.mockResolvedValue("/home/user/Downloads");
    mocks.join.mockImplementation((...parts: string[]) => Promise.resolve(parts.join("/")));
    mocks.writeTextFile.mockResolvedValue({ status: "ok", data: null });
    mocks.revealItemInDir.mockResolvedValue({ status: "ok", data: null });

    const raw = JSON.stringify([
      "Notare",
      { wrong: "far eye", right: "FarEye", caseSensitive: true },
    ]);
    render(<DictionarySettings raw={raw} onSave={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => expect(mocks.writeTextFile).toHaveBeenCalled());
    const [path, content] = mocks.writeTextFile.mock.calls[0];
    expect(path).toMatch(/^\/home\/user\/Downloads\/notare-dictionary_.*\.txt$/);
    expect(content).toBe("Notare\nfar eye => FarEye [cs]");
    expect(mocks.revealItemInDir).toHaveBeenCalledWith(path);
  });

  it("shows an export error instead of crashing when the write fails", async () => {
    mocks.downloadDir.mockResolvedValue("/home/user/Downloads");
    mocks.join.mockImplementation((...parts: string[]) => Promise.resolve(parts.join("/")));
    mocks.writeTextFile.mockResolvedValue({ status: "error", error: "disk full" });

    render(<DictionarySettings raw={JSON.stringify(["Notare"])} onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() =>
      expect(screen.getByText(/Export failed: disk full/)).toBeTruthy(),
    );
    expect(mocks.revealItemInDir).not.toHaveBeenCalled();
  });

  it("falls back to an empty dictionary on unparseable stored data", () => {
    render(<DictionarySettings raw="not json" onSave={vi.fn()} />);
    expect(screen.getByText("No dictionary entries yet.")).toBeTruthy();
  });
});

describe("mergeDictionaryEntries", () => {
  it("adds new entries and updates existing ones by wrong/term key, case-insensitively", () => {
    const existing = ["Notare", { wrong: "far eye", right: "FarEye", caseSensitive: false }];
    const incoming = [
      "notare", // same key as existing "Notare"; identical after normalization -> not counted as update
      { wrong: "Far Eye", right: "FarEye Corp", caseSensitive: true },
      "NewTerm",
    ];

    const { merged, addedCount, updatedCount } = mergeDictionaryEntries(
      existing,
      incoming,
    );

    expect(addedCount).toBe(1);
    expect(updatedCount).toBe(1);
    expect(merged).toEqual([
      "Notare",
      { wrong: "Far Eye", right: "FarEye Corp", caseSensitive: true },
      "NewTerm",
    ]);
  });

  it("never wipes existing entries when incoming is empty", () => {
    const existing = ["Notare"];
    const { merged, addedCount, updatedCount } = mergeDictionaryEntries(existing, []);

    expect(merged).toEqual(["Notare"]);
    expect(addedCount).toBe(0);
    expect(updatedCount).toBe(0);
  });

  // Importing a plain term list into a mapping-rich dictionary must not
  // downgrade wrong->right rewrites to bare hint terms.
  it("keeps an existing mapping when an incoming flat term collides with it", () => {
    const mapping = { wrong: "far eye", right: "FarEye", caseSensitive: false };
    const { merged, addedCount, updatedCount } = mergeDictionaryEntries(
      [mapping],
      ["Far Eye", "NewTerm"],
    );

    expect(merged).toEqual([mapping, "NewTerm"]);
    expect(addedCount).toBe(1);
    expect(updatedCount).toBe(0);
  });

  // Space-variant duplicates ("Far  Eye" vs "Far Eye") share one key.
  it("treats whitespace-collapsed keys as the same entry", () => {
    const { merged, addedCount } = mergeDictionaryEntries(
      [{ wrong: "Far Eye", right: "FarEye", caseSensitive: false }],
      [{ wrong: "Far  Eye", right: "FarEye", caseSensitive: false }],
    );

    expect(merged).toHaveLength(1);
    expect(addedCount).toBe(0);
  });
});
