import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  DICTATION_HISTORY_RETENTION_OPTIONS,
  HistoryRetentionRow,
  normalizeHistoryRetention,
} from "./retention";

describe("normalizeHistoryRetention", () => {
  it("passes through every known retention value", () => {
    for (const value of DICTATION_HISTORY_RETENTION_OPTIONS) {
      expect(normalizeHistoryRetention(value)).toBe(value);
    }
  });

  it('falls back to "off" for undefined or an unrecognized value', () => {
    expect(normalizeHistoryRetention(undefined)).toBe("off");
    expect(normalizeHistoryRetention("legacy-forever")).toBe("off");
  });
});

describe("HistoryRetentionRow", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows the current selection and pinned-exempt copy", () => {
    render(<HistoryRetentionRow value="30d" onChange={vi.fn()} />);

    expect(screen.getByText("30 days")).toBeTruthy();
    expect(screen.getByText(/Pinned snippets are always kept/)).toBeTruthy();
  });

  it('defaults an unset value to "Keep everything"', () => {
    render(<HistoryRetentionRow value="" onChange={vi.fn()} />);

    expect(screen.getByText("Keep everything")).toBeTruthy();
  });

  it("reports the newly picked retention window", () => {
    const onChange = vi.fn();
    render(<HistoryRetentionRow value="off" onChange={onChange} />);

    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.click(screen.getByText("7 days"));

    expect(onChange).toHaveBeenCalledWith("7d");
  });
});
