import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

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

import { SummaryInstructionsSettings } from "./index";

describe("SummaryInstructionsSettings", () => {
  afterEach(() => {
    cleanup();
  });

  it("explains that instructions take priority over conflicting templates", () => {
    render(
      <SummaryInstructionsSettings
        instructions="Keep it brief"
        onSave={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        /These instructions take priority over the selected template when they conflict/,
      ),
    ).toBeTruthy();
    expect(
      (
        screen.getByRole("textbox", {
          name: "Summary instructions",
        }) as HTMLTextAreaElement
      ).value,
    ).toBe("Keep it brief");
  });

  it("saves trimmed instructions explicitly", async () => {
    const onSave = vi.fn();
    render(<SummaryInstructionsSettings instructions="" onSave={onSave} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Summary instructions" }),
      { target: { value: "  Use a short executive summary.  " } },
    );

    const saveButton = screen.getByRole("button", {
      name: "Save",
    }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith("Use a short executive summary."),
    );
  });

  it("resets saved instructions to the built-in behavior", () => {
    const onSave = vi.fn();
    render(
      <SummaryInstructionsSettings
        instructions="Use a table"
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset to default" }));

    expect(onSave).toHaveBeenCalledWith("");
    expect(
      (
        screen.getByRole("textbox", {
          name: "Summary instructions",
        }) as HTMLTextAreaElement
      ).value,
    ).toBe("");
  });
});

// DictionarySettings now lives in ./dictionary-settings.tsx (upgraded to
// support wrong->right mappings alongside flat terms); its tests live in
// ./dictionary-settings.test.tsx.
