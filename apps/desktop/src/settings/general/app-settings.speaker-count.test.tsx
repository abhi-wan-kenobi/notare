import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  configValues: {} as Record<string, unknown>,
  setSettingValue: vi.fn(),
  clearSettingValue: vi.fn(),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => (value: unknown) => {
    mocks.configValues.diarization_speaker_count = value;
    mocks.setSettingValue(value);
  },
  useClearSettingValue: () => () => {
    delete mocks.configValues.diarization_speaker_count;
    mocks.clearSettingValue();
  },
}));

import { DiarizationSpeakerCountRow } from "./app-settings";

const NUMBER_INPUT_LABEL = "Number of speakers";

function renderRow() {
  return render(<DiarizationSpeakerCountRow />);
}

function getNumberInput(): HTMLInputElement {
  return screen.getByLabelText(NUMBER_INPUT_LABEL) as HTMLInputElement;
}

describe("DiarizationSpeakerCountRow", () => {
  beforeEach(() => {
    mocks.configValues = {};
    mocks.setSettingValue.mockReset();
    mocks.clearSettingValue.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders no number input while in Automatic mode", () => {
    renderRow();

    expect(screen.queryByLabelText(NUMBER_INPUT_LABEL)).toBeNull();
  });

  it("persists a valid manual count typed in the input", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    expect(input.value).toBe("3");

    fireEvent.change(input, { target: { value: "5" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).toHaveBeenCalledTimes(1);
    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(5);
    expect(getNumberInput().value).toBe("5");
  });

  it("clamps a below-range value (0) to 1 and reflects it back", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(1);
    expect(getNumberInput().value).toBe("1");
  });

  it("clamps a negative value to 1 and reflects it back", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "-2" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(1);
    expect(getNumberInput().value).toBe("1");
  });

  it("clamps an above-range value (15) to 10 and reflects it back", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "15" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(10);
    expect(getNumberInput().value).toBe("10");
  });

  it("reverts the input to the persisted value when given non-numeric text", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "abc" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).not.toHaveBeenCalled();
    expect(getNumberInput().value).toBe("3");
  });

  it("reverts the input to the persisted value when cleared via blur", () => {
    mocks.configValues.diarization_speaker_count = 3;

    renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.blur(input);

    expect(mocks.setSettingValue).not.toHaveBeenCalled();
    expect(getNumberInput().value).toBe("3");
  });

  it("writes the default count (2) when Manual is clicked from Automatic", () => {
    const { rerender } = renderRow();

    fireEvent.click(screen.getByRole("button", { name: "Manual" }));

    // The mocked set/clear updates the config store but doesn't itself
    // schedule a React re-render, so we re-render to observe the new
    // persisted value before querying the input.
    rerender(<DiarizationSpeakerCountRow />);

    expect(mocks.setSettingValue).toHaveBeenCalledTimes(1);
    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(2);
    expect(getNumberInput().value).toBe("2");
  });

  it("Automatic → Manual round-trip does not resurrect a stale local count", () => {
    // Mount in Manual mode with persisted value 5, then change the local
    // input to 9 (simulating a typing that hasn't blurred), then switch
    // back to Automatic, then click Manual again. The Manual click must
    // commit the default (2), not the stale 9 left in local state.
    mocks.configValues.diarization_speaker_count = 5;

    const { rerender } = renderRow();

    const input = getNumberInput();
    fireEvent.change(input, { target: { value: "9" } });
    // Switch to Automatic.
    fireEvent.click(screen.getByRole("button", { name: "Automatic" }));
    expect(mocks.clearSettingValue).toHaveBeenCalledTimes(1);

    // The clear mock deleted the persisted value; re-render so the row
    // observes isManual === false before the next click.
    rerender(<DiarizationSpeakerCountRow />);
    expect(screen.queryByLabelText(NUMBER_INPUT_LABEL)).toBeNull();

    // Click Manual again — must commit 2, not 9.
    fireEvent.click(screen.getByRole("button", { name: "Manual" }));
    rerender(<DiarizationSpeakerCountRow />);

    expect(mocks.setSettingValue).toHaveBeenCalledTimes(1);
    expect(mocks.setSettingValue).toHaveBeenLastCalledWith(2);

    // The re-mounted input must display the freshly committed value.
    expect(getNumberInput().value).toBe("2");
  });
});
