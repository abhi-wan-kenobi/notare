import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TranscriptEmptyState } from "./empty";

describe("TranscriptEmptyState", () => {
  afterEach(() => {
    cleanup();
  });

  it("lets users stop batch transcription", () => {
    const onStopTranscription = vi.fn();

    render(
      <TranscriptEmptyState
        isBatching
        phase="transcribing"
        onStopTranscription={onStopTranscription}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop transcription" }));

    expect(onStopTranscription).toHaveBeenCalledTimes(1);
  });

  it("hides the stop control while importing audio", () => {
    render(<TranscriptEmptyState isBatching phase="importing" />);

    expect(
      screen.queryByRole("button", { name: "Stop transcription" }),
    ).toBeNull();
  });

  it("renders fractional batch progress as a percentage", () => {
    render(
      <TranscriptEmptyState isBatching phase="importing" percentage={0.42} />,
    );

    expect(screen.getByText("42%")).not.toBeNull();
  });
});
